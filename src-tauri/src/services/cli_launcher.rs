use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::Value;

use crate::app_config::AppType;
use crate::database::Database;
use crate::error::AppError;
use crate::provider::Provider;

#[derive(Debug, Clone)]
pub enum CliLaunchError {
    EmptyProviderQuery,
    ProviderNotFound {
        query: String,
        available: Vec<ProviderSummary>,
    },
    AmbiguousProvider {
        query: String,
        matches: Vec<ProviderSummary>,
    },
    InvalidCwd(String),
    Config(String),
    Process(String),
}

impl fmt::Display for CliLaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderQuery => write!(f, "供应商不能为空"),
            Self::ProviderNotFound { query, available } => {
                write!(f, "未找到供应商: {query}")?;
                if !available.is_empty() {
                    write!(f, "\n可用供应商:")?;
                    for provider in available {
                        write!(f, "\n  {}\t{}", provider.name, provider.id)?;
                    }
                }
                write!(
                    f,
                    "\n提示:\n  1. 输入 `cc-switch providers claude` 查看完整列表\n  2. 输入 `cc-switch setup shell` 安装 Tab 补全"
                )?;
                Ok(())
            }
            Self::AmbiguousProvider { query, matches } => {
                write!(f, "供应商匹配不唯一: {query}\n候选:")?;
                for provider in matches {
                    write!(f, "\n  {}\t{}", provider.name, provider.id)?;
                }
                write!(f, "\n提示: 使用完整 provider 名称或 ID。")?;
                Ok(())
            }
            Self::InvalidCwd(message) => write!(f, "{message}"),
            Self::Config(message) => write!(f, "{message}"),
            Self::Process(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliLaunchError {}

impl From<AppError> for CliLaunchError {
    fn from(value: AppError) -> Self {
        Self::Config(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSlotInfo {
    pub name: String,
    pub env_key: &'static str,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaudeLaunchOptions {
    pub provider: String,
    pub explicit_model: Option<String>,
    pub cwd: Option<String>,
    pub passthrough: Vec<String>,
}

const MODEL_SLOTS: &[(&str, &str)] = &[
    ("default", "ANTHROPIC_MODEL"),
    ("haiku", "ANTHROPIC_DEFAULT_HAIKU_MODEL"),
    ("sonnet", "ANTHROPIC_DEFAULT_SONNET_MODEL"),
    ("opus", "ANTHROPIC_DEFAULT_OPUS_MODEL"),
];

pub fn match_provider(
    providers: &IndexMap<String, Provider>,
    query: &str,
) -> Result<Provider, CliLaunchError> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return Err(CliLaunchError::EmptyProviderQuery);
    }

    if let Some(provider) = providers.get(normalized) {
        return Ok(provider.clone());
    }

    let lowered = normalized.to_lowercase();
    let matches = providers
        .values()
        .filter(|provider| provider.name.trim().to_lowercase() == lowered)
        .map(provider_summary)
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [single] => {
            providers
                .get(&single.id)
                .cloned()
                .ok_or_else(|| CliLaunchError::ProviderNotFound {
                    query: normalized.to_string(),
                    available: provider_summaries(providers),
                })
        }
        [] => Err(CliLaunchError::ProviderNotFound {
            query: normalized.to_string(),
            available: provider_summaries(providers),
        }),
        _ => Err(CliLaunchError::AmbiguousProvider {
            query: normalized.to_string(),
            matches,
        }),
    }
}

pub fn build_claude_settings(
    provider: &Provider,
    explicit_model: Option<&str>,
) -> Result<Value, CliLaunchError> {
    let mut settings =
        crate::services::provider::sanitize_claude_settings_for_live(&provider.settings_config);
    if let Some(model) = explicit_model.and_then(clean_model_id) {
        let root = settings
            .as_object_mut()
            .ok_or_else(|| CliLaunchError::ProviderNotFound {
                query: provider.id.clone(),
                available: Vec::new(),
            })?;
        let env = root
            .entry("env")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if !env.is_object() {
            *env = Value::Object(serde_json::Map::new());
        }
        if let Some(env_obj) = env.as_object_mut() {
            env_obj.insert("ANTHROPIC_MODEL".to_string(), Value::String(model));
        }
    }
    Ok(settings)
}

pub fn describe_model_slots(provider: &Provider) -> Vec<ModelSlotInfo> {
    MODEL_SLOTS
        .iter()
        .map(|(name, env_key)| ModelSlotInfo {
            name: (*name).to_string(),
            env_key,
            model: provider
                .settings_config
                .get("env")
                .and_then(Value::as_object)
                .and_then(|env| env.get(*env_key))
                .and_then(Value::as_str)
                .and_then(clean_model_id),
        })
        .collect()
}

pub fn run_claude_with_provider(options: ClaudeLaunchOptions) -> Result<i32, CliLaunchError> {
    let db = Database::init()?;
    let providers = db.get_all_providers(AppType::Claude.as_str())?;
    let provider = match_provider(&providers, &options.provider)?;
    let settings = build_claude_settings(&provider, options.explicit_model.as_deref())?;
    let cwd = resolve_cwd(options.cwd.as_deref())?;
    let settings_path = write_temp_claude_settings(&provider.id, &settings)?;

    let result = spawn_claude(&settings_path, cwd.as_deref(), &options.passthrough);
    let _ = std::fs::remove_file(&settings_path);
    result
}

fn provider_summary(provider: &Provider) -> ProviderSummary {
    ProviderSummary {
        id: provider.id.clone(),
        name: provider.name.clone(),
    }
}

fn provider_summaries(providers: &IndexMap<String, Provider>) -> Vec<ProviderSummary> {
    providers.values().map(provider_summary).collect()
}

fn resolve_cwd(raw: Option<&str>) -> Result<Option<PathBuf>, CliLaunchError> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if raw.contains('\n') || raw.contains('\r') {
        return Err(CliLaunchError::InvalidCwd(
            "目录路径包含非法换行符".to_string(),
        ));
    }
    let path = Path::new(raw);
    if !path.exists() {
        return Err(CliLaunchError::InvalidCwd(format!("目录不存在: {raw}")));
    }
    let resolved = std::fs::canonicalize(path)
        .map_err(|e| CliLaunchError::InvalidCwd(format!("解析目录失败: {e}")))?;
    if !resolved.is_dir() {
        return Err(CliLaunchError::InvalidCwd(format!(
            "选择的路径不是文件夹: {}",
            resolved.display()
        )));
    }
    Ok(Some(resolved))
}

fn write_temp_claude_settings(
    provider_id: &str,
    settings: &Value,
) -> Result<PathBuf, CliLaunchError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "cc-switch-claude-{}-{}-{ts}.json",
        sanitize_temp_name(provider_id),
        std::process::id()
    ));
    crate::config::write_json_file(&path, settings)?;
    Ok(path)
}

fn spawn_claude(
    settings_path: &Path,
    cwd: Option<&Path>,
    passthrough: &[String],
) -> Result<i32, CliLaunchError> {
    let mut command = Command::new("claude");
    command
        .arg("--settings")
        .arg(settings_path)
        .args(passthrough)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let status = command.status().map_err(|e| {
        CliLaunchError::Process(format!(
            "启动 claude 失败，请确认 Claude Code 已安装且在 PATH 中: {e}"
        ))
    })?;
    Ok(status.code().unwrap_or(1))
}

fn sanitize_temp_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn clean_model_id(raw: &str) -> Option<String> {
    let mut cleaned = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            continue;
        }
        if !ch.is_control() {
            cleaned.push(ch);
        }
    }
    let cleaned = strip_literal_ansi_fragments(&cleaned);
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn strip_literal_ansi_fragments(raw: &str) -> String {
    let mut cleaned = String::new();
    let mut chars = raw.chars().peekable();
    'outer: while let Some(ch) = chars.next() {
        if ch == '[' {
            let lookahead = chars.clone();
            let mut saw_code = false;
            for next in lookahead {
                if next.is_ascii_digit() || next == ';' {
                    saw_code = true;
                    continue;
                }
                if next == 'm' && saw_code {
                    for consumed in chars.by_ref() {
                        if consumed == 'm' {
                            break;
                        }
                    }
                    if matches!(chars.peek(), Some(']')) {
                        chars.next();
                    }
                    continue 'outer;
                }
                break;
            }
        }
        cleaned.push(ch);
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use serde_json::{json, Value};

    use crate::provider::Provider;

    fn test_provider(id: &str, name: &str, settings_config: serde_json::Value) -> Provider {
        Provider {
            id: id.to_string(),
            name: name.to_string(),
            settings_config,
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: None,
            icon: None,
            icon_color: None,
            in_failover_queue: false,
        }
    }

    fn test_providers(items: Vec<(&str, &str)>) -> IndexMap<String, Provider> {
        items
            .into_iter()
            .map(|(id, name)| {
                (
                    id.to_string(),
                    test_provider(id, name, json!({ "env": { "ANTHROPIC_MODEL": "model" } })),
                )
            })
            .collect()
    }

    #[test]
    fn match_provider_prefers_id_over_name() {
        let providers = test_providers(vec![("deepseek", "DeepSeek"), ("other", "deepseek")]);
        let matched = super::match_provider(&providers, "deepseek").expect("match provider");
        assert_eq!(matched.id, "deepseek");
    }

    #[test]
    fn describe_model_slots_reads_default_haiku_sonnet_and_opus() {
        let provider = test_provider(
            "deepseek",
            "DeepSeek",
            json!({
                "env": {
                    "ANTHROPIC_MODEL": "deepseek-v4-default",
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "deepseek-v4-flash",
                    "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-v4-pro",
                    "ANTHROPIC_DEFAULT_OPUS_MODEL": "deepseek-v4-opus"
                }
            }),
        );

        let slots = super::describe_model_slots(&provider);
        let pairs = slots
            .iter()
            .map(|slot| (slot.name.as_str(), slot.model.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            pairs,
            vec![
                ("default", Some("deepseek-v4-default")),
                ("haiku", Some("deepseek-v4-flash")),
                ("sonnet", Some("deepseek-v4-pro")),
                ("opus", Some("deepseek-v4-opus")),
            ]
        );
    }

    #[test]
    fn describe_model_slots_strips_ansi_suffixes() {
        let provider = test_provider(
            "deepseek",
            "DeepSeek",
            json!({
                "env": {
                    "ANTHROPIC_MODEL": "deepseek-v4-pro\u{1b}[1m",
                    "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-v4-flash[1m]"
                }
            }),
        );

        let slots = super::describe_model_slots(&provider);

        assert_eq!(slots[0].model.as_deref(), Some("deepseek-v4-pro"));
        assert_eq!(slots[2].model.as_deref(), Some("deepseek-v4-flash"));
    }

    #[test]
    fn explicit_model_overrides_default_model() {
        let provider = test_provider(
            "deepseek",
            "DeepSeek",
            json!({
                "env": {
                    "ANTHROPIC_MODEL": "deepseek-v4-pro"
                }
            }),
        );

        let settings =
            super::build_claude_settings(&provider, Some("custom-model")).expect("build settings");

        assert_eq!(
            settings
                .pointer("/env/ANTHROPIC_MODEL")
                .and_then(Value::as_str),
            Some("custom-model")
        );
    }

    #[test]
    fn build_settings_removes_internal_claude_fields() {
        let provider = test_provider(
            "deepseek",
            "DeepSeek",
            json!({
                "api_format": "openai_chat",
                "openrouterCompatMode": true,
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                    "ANTHROPIC_AUTH_TOKEN": "secret",
                    "ANTHROPIC_MODEL": "deepseek-v4-pro"
                }
            }),
        );

        let settings = super::build_claude_settings(&provider, None).expect("build settings");

        assert!(settings.get("api_format").is_none());
        assert!(settings.get("openrouterCompatMode").is_none());
        assert_eq!(
            settings
                .pointer("/env/ANTHROPIC_MODEL")
                .and_then(Value::as_str),
            Some("deepseek-v4-pro")
        );
    }

    #[test]
    fn provider_not_found_display_includes_actionable_hints() {
        let err = super::CliLaunchError::ProviderNotFound {
            query: "deep".to_string(),
            available: vec![super::ProviderSummary {
                id: "deepseek-id".to_string(),
                name: "DeepSeek".to_string(),
            }],
        }
        .to_string();

        assert!(err.contains("cc-switch providers claude"));
        assert!(err.contains("cc-switch setup shell"));
    }
}
