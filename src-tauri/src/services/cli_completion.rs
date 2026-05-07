use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::services::cli_launcher::ProviderSummary;
use crate::{app_config::AppType, database::Database};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub value: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Zsh,
    Bash,
}

impl ShellKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "bash",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSetupReport {
    pub shell: String,
    pub rc_path: String,
    pub backup_path: Option<String>,
}

const ROOT_COMMANDS: &[(&str, &str)] = &[
    ("claude", "启动 Claude Code provider 终端"),
    ("providers", "查看 provider"),
    ("current", "查看当前 provider"),
    ("models", "查看 Claude 模型位"),
    ("help", "查看帮助"),
    ("completion", "输出 shell 补全脚本"),
    ("setup", "安装 shell 集成"),
    ("doctor", "诊断 shell 集成"),
];

const CLAUDE_FLAGS: &[(&str, &str)] = &[
    ("--model", "指定模型 ID"),
    ("--cwd", "指定工作目录"),
    ("--", "透传后续参数给 Claude Code"),
];

const MACOS_BASH_LOGIN_RC_FILES: &[&str] = &[".bash_profile", ".bash_login", ".profile"];

pub fn complete_with_provider_summaries(
    tokens: &[String],
    providers: &[ProviderSummary],
) -> Result<Vec<CompletionCandidate>, String> {
    let current = tokens.last().map(String::as_str).unwrap_or("");
    let command = tokens.first().map(String::as_str).unwrap_or("");

    match command {
        "" => Ok(match_static_candidates(ROOT_COMMANDS, current)),
        "claude" => complete_claude(tokens, providers),
        "models" => complete_models(tokens, providers),
        "completion" => Ok(match_static_candidates(
            &[("zsh", "zsh 补全脚本"), ("bash", "bash 补全脚本")],
            current,
        )),
        "setup" => Ok(match_static_candidates(
            &[("shell", "安装补全"), ("alias", "安装短别名")],
            current,
        )),
        "doctor" => Ok(match_static_candidates(
            &[("shell", "诊断 shell 集成")],
            current,
        )),
        _ => Ok(match_static_candidates(ROOT_COMMANDS, current)),
    }
}

pub fn complete(tokens: &[String]) -> Result<Vec<CompletionCandidate>, String> {
    let providers = load_claude_provider_summaries().unwrap_or_default();
    complete_with_provider_summaries(tokens, &providers)
}

fn complete_claude(
    tokens: &[String],
    providers: &[ProviderSummary],
) -> Result<Vec<CompletionCandidate>, String> {
    match tokens.len() {
        0..=2 => {
            let current = current_token(tokens);
            Ok(match_provider_candidates(providers, current))
        }
        _ => {
            let current = current_token(tokens);
            Ok(match_static_candidates(CLAUDE_FLAGS, current))
        }
    }
}

fn complete_models(
    tokens: &[String],
    providers: &[ProviderSummary],
) -> Result<Vec<CompletionCandidate>, String> {
    match tokens.get(1).map(String::as_str) {
        None | Some("") => Ok(match_static_candidates(&[("claude", "Claude app")], "")),
        Some("claude") => {
            let current = current_token(tokens);
            Ok(match_provider_candidates(providers, current))
        }
        Some(current) => Ok(match_static_candidates(
            &[("claude", "Claude app")],
            current,
        )),
    }
}

fn current_token(tokens: &[String]) -> &str {
    tokens.last().map(String::as_str).unwrap_or("")
}

fn match_static_candidates(candidates: &[(&str, &str)], prefix: &str) -> Vec<CompletionCandidate> {
    let prefix = prefix.to_lowercase();
    candidates
        .iter()
        .filter(|(value, _)| value.to_lowercase().starts_with(&prefix))
        .map(|(value, description)| CompletionCandidate {
            value: (*value).to_string(),
            description: (*description).to_string(),
        })
        .collect()
}

fn match_provider_candidates(
    providers: &[ProviderSummary],
    prefix: &str,
) -> Vec<CompletionCandidate> {
    let prefix = prefix.to_lowercase();
    providers
        .iter()
        .filter(|provider| {
            provider.name.to_lowercase().starts_with(&prefix)
                || provider.id.to_lowercase().starts_with(&prefix)
        })
        .map(|provider| CompletionCandidate {
            value: provider.name.clone(),
            description: "provider".to_string(),
        })
        .collect()
}

pub fn generate_completion_script(shell: ShellKind, command_name: &str, exe_path: &str) -> String {
    match shell {
        ShellKind::Zsh => generate_zsh_completion_script(command_name, exe_path),
        ShellKind::Bash => generate_bash_completion_script(command_name, exe_path),
    }
}

fn generate_zsh_completion_script(command_name: &str, exe_path: &str) -> String {
    let command = shell_escape(command_name);
    let exe = shell_escape(exe_path);
    format!(
        r#"#compdef {command_name}
_cc_switch() {{
  local -a completions
  local line value description
  while IFS=$'\t' read -r value description; do
    [[ -z "$value" ]] && continue
    if [[ -n "$description" ]]; then
      completions+=("$value:$description")
    else
      completions+=("$value")
    fi
  done < <({exe} __complete "${{words[@]:1}}")
  _describe 'cc-switch' completions
}}
compdef _cc_switch {command} ccs
"#,
        command_name = command_name,
        command = command,
        exe = exe,
    )
}

fn generate_bash_completion_script(command_name: &str, exe_path: &str) -> String {
    let command = shell_escape(command_name);
    let exe = shell_escape(exe_path);
    format!(
        r#"_cc_switch() {{
  local value description
  COMPREPLY=()
  while IFS=$'\t' read -r value description; do
    [[ -z "$value" ]] && continue
    COMPREPLY+=("$value")
  done < <({exe} __complete "${{COMP_WORDS[@]:1}}")
  compopt -o nospace 2>/dev/null || true
}}
complete -F _cc_switch {command} ccs
"#,
        command = command,
        exe = exe,
    )
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn upsert_marker_block(existing: &str, marker: &str, body: &str) -> String {
    let start = marker_start(marker);
    let end = marker_end(marker);
    let normalized_body = ensure_trailing_newline(body);
    let block = format!("{start}\n{normalized_body}{end}\n");

    if let Some(start_idx) = existing.find(&start) {
        if let Some(end_rel_idx) = existing[start_idx..].find(&end) {
            let end_idx = start_idx + end_rel_idx + end.len();
            let after_end = if existing[end_idx..].starts_with('\n') {
                end_idx + 1
            } else {
                end_idx
            };
            let mut updated = String::new();
            updated.push_str(&existing[..start_idx]);
            updated.push_str(&block);
            updated.push_str(&existing[after_end..]);
            return updated;
        }
    }

    let mut updated = existing.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str(&block);
    updated
}

#[cfg(test)]
pub fn build_alias_marker_block(
    alias_name: &str,
    exe_path: &str,
    command_exists: bool,
    force: bool,
) -> Result<String, String> {
    let body = build_alias_body(alias_name, exe_path, command_exists, force)?;
    Ok(upsert_marker_block(
        "",
        &format!("alias {alias_name}"),
        &body,
    ))
}

pub fn install_shell_completion() -> Result<ShellSetupReport, String> {
    let shell = detect_current_shell()?;
    let exe_path = current_exe_path()?;
    let body = format!(
        "eval \"$({} completion {})\"\n",
        shell_escape(&exe_path),
        shell.as_str()
    );
    write_rc_marker(shell, "completion", &body)
}

pub fn install_alias(alias_name: &str, force: bool) -> Result<ShellSetupReport, String> {
    let shell = detect_current_shell()?;
    let exe_path = current_exe_path()?;
    let body = build_alias_body(alias_name, &exe_path, command_exists(alias_name), force)?;
    write_rc_marker(shell, &format!("alias {alias_name}"), &body)
}

pub fn doctor_shell() -> Result<String, String> {
    let shell_env = std::env::var("SHELL").unwrap_or_else(|_| "-".to_string());
    let exe_path = current_exe_path().unwrap_or_else(|_| "-".to_string());
    let shell = detect_current_shell();
    let mut lines = vec![
        "cc-switch shell 诊断".to_string(),
        format!("SHELL: {shell_env}"),
        format!("cc-switch: {exe_path}"),
    ];

    match shell {
        Ok(shell) => {
            let rc_path = default_rc_path(shell)?;
            let rc_display = rc_path.display().to_string();
            let rc_content = std::fs::read_to_string(&rc_path).unwrap_or_default();
            let completion_installed = rc_content.contains(&marker_start("completion"));
            let ccs_alias_installed = rc_content.contains(&marker_start("alias ccs"));
            let cc_alias_installed = rc_content.contains(&marker_start("alias cc"));

            lines.push(format!("shell 类型: {}", shell.as_str()));
            lines.push(format!("配置文件: {rc_display}"));
            lines.push(format!(
                "Tab 补全: {}",
                if completion_installed {
                    "已安装"
                } else {
                    "未安装，运行 `cc-switch setup shell`"
                }
            ));
            lines.push(format!(
                "推荐短命令 ccs: {}",
                if ccs_alias_installed {
                    "已安装"
                } else {
                    "未安装，运行 `cc-switch setup alias ccs`"
                }
            ));
            let cc_status = if cc_alias_installed {
                "已由 cc-switch 安装"
            } else if let Some(path) = command_location("cc") {
                lines.push(format!("系统 cc 当前指向: {path}"));
                "未覆盖，如需强制覆盖运行 `cc-switch setup alias cc --force`"
            } else {
                "未安装"
            };
            lines.push(format!("短命令 cc: {cc_status}"));
        }
        Err(err) => {
            lines.push(format!("shell 类型: {err}"));
            lines.push("当前只支持 zsh 和 bash。".to_string());
        }
    }

    Ok(lines.join("\n"))
}

fn build_alias_body(
    alias_name: &str,
    exe_path: &str,
    command_exists: bool,
    force: bool,
) -> Result<String, String> {
    validate_alias_name(alias_name)?;
    if alias_name == "cc" && command_exists && !force {
        return Err(
            "短命令 cc 当前指向系统编译器，不建议覆盖。\n推荐使用:\n  cc-switch setup alias ccs\n如仍要覆盖:\n  cc-switch setup alias cc --force"
                .to_string(),
        );
    }

    Ok(format!("alias {}={}\n", alias_name, shell_escape(exe_path),))
}

fn marker_start(marker: &str) -> String {
    format!("# >>> cc-switch {marker} >>>")
}

fn marker_end(marker: &str) -> String {
    format!("# <<< cc-switch {marker} <<<")
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn validate_alias_name(alias_name: &str) -> Result<(), String> {
    let valid = !alias_name.is_empty()
        && alias_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(format!("别名不合法: {alias_name}"))
    }
}

fn load_claude_provider_summaries() -> Result<Vec<ProviderSummary>, String> {
    let db = Database::init().map_err(|e| e.to_string())?;
    let providers = db
        .get_all_providers(AppType::Claude.as_str())
        .map_err(|e| e.to_string())?;
    Ok(providers
        .values()
        .map(|provider| ProviderSummary {
            id: provider.id.clone(),
            name: provider.name.clone(),
        })
        .collect())
}

fn detect_current_shell() -> Result<ShellKind, String> {
    let raw_shell = std::env::var("SHELL").map_err(|_| {
        "无法检测当前 shell，请设置 SHELL 或手动使用 `cc-switch completion zsh|bash`".to_string()
    })?;
    parse_shell_path(&raw_shell)
}

fn parse_shell_path(raw_shell: &str) -> Result<ShellKind, String> {
    let shell_name = Path::new(raw_shell)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(raw_shell);
    match shell_name {
        "zsh" => Ok(ShellKind::Zsh),
        "bash" => Ok(ShellKind::Bash),
        other => Err(format!("不支持的 shell: {other}，可用: zsh, bash")),
    }
}

fn default_rc_path(shell: ShellKind) -> Result<PathBuf, String> {
    Ok(default_rc_path_for_home(shell, &home_dir()?))
}

fn default_rc_path_for_home(shell: ShellKind, home: &Path) -> PathBuf {
    match shell {
        ShellKind::Zsh => home.join(".zshrc"),
        ShellKind::Bash => bash_rc_path_for_home(home),
    }
}

fn bash_rc_path_for_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        for file_name in MACOS_BASH_LOGIN_RC_FILES {
            let candidate = home.join(file_name);
            if candidate.exists() {
                return candidate;
            }
        }
        return home.join(".bash_profile");
    }

    home.join(".bashrc")
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "无法检测 HOME 目录".to_string())
}

fn current_exe_path() -> Result<String, String> {
    std::env::current_exe()
        .map_err(|e| format!("获取当前可执行文件路径失败: {e}"))
        .map(|path| path.to_string_lossy().to_string())
}

fn write_rc_marker(shell: ShellKind, marker: &str, body: &str) -> Result<ShellSetupReport, String> {
    let rc_path = default_rc_path(shell)?;
    if let Some(parent) = rc_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }

    let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();
    let updated = upsert_marker_block(&existing, marker, body);
    if updated == existing {
        return Ok(ShellSetupReport {
            shell: shell.as_str().to_string(),
            rc_path: rc_path.display().to_string(),
            backup_path: None,
        });
    }

    let backup_path = if rc_path.exists() {
        let backup_path = backup_path_for(&rc_path);
        std::fs::copy(&rc_path, &backup_path).map_err(|e| format!("备份配置文件失败: {e}"))?;
        Some(backup_path.display().to_string())
    } else {
        None
    };
    std::fs::write(&rc_path, updated).map_err(|e| format!("写入配置文件失败: {e}"))?;
    Ok(ShellSetupReport {
        shell: shell.as_str().to_string(),
        rc_path: rc_path.display().to_string(),
        backup_path,
    })
}

fn backup_path_for(path: &Path) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    backup_path_for_timestamp(path, ts)
}

fn backup_path_for_timestamp(path: &Path, timestamp: u128) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("shellrc");
    let base_name = format!("{file_name}.cc-switch-backup-{timestamp}");
    let first = path.with_file_name(&base_name);
    if !first.exists() {
        return first;
    }

    let mut attempt = 1;
    loop {
        let candidate = path.with_file_name(format!("{base_name}-{attempt}"));
        if !candidate.exists() {
            return candidate;
        }
        attempt += 1;
    }
}

fn command_exists(command: &str) -> bool {
    command_location(command).is_some()
}

fn command_location(command: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {}", shell_escape(command)))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::services::cli_launcher::ProviderSummary;
    #[cfg(target_os = "macos")]
    use serial_test::serial;

    fn providers() -> Vec<ProviderSummary> {
        vec![
            ProviderSummary {
                id: "deepseek-id".to_string(),
                name: "DeepSeek".to_string(),
            },
            ProviderSummary {
                id: "kimi-id".to_string(),
                name: "Kimi For Coding".to_string(),
            },
        ]
    }

    #[test]
    fn completes_root_commands() {
        let candidates = super::complete_with_provider_summaries(&["".to_string()], &providers())
            .expect("complete root");
        let values = candidates
            .iter()
            .map(|candidate| candidate.value.as_str())
            .collect::<Vec<_>>();

        assert!(values.contains(&"claude"));
        assert!(values.contains(&"completion"));
        assert!(values.contains(&"setup"));
        assert!(values.contains(&"doctor"));
    }

    #[test]
    fn completes_provider_names_by_prefix() {
        let candidates = super::complete_with_provider_summaries(
            &["claude".to_string(), "Deep".to_string()],
            &providers(),
        )
        .expect("complete providers");

        assert_eq!(candidates[0].value, "DeepSeek");
        assert_eq!(candidates[0].description, "provider");
    }

    #[test]
    fn completes_flags_after_provider() {
        let candidates = super::complete_with_provider_summaries(
            &["claude".to_string(), "DeepSeek".to_string(), "".to_string()],
            &providers(),
        )
        .expect("complete flags");
        let values = candidates
            .iter()
            .map(|candidate| candidate.value.as_str())
            .collect::<Vec<_>>();

        assert!(!values.contains(&"flash"));
        assert!(!values.contains(&"pro"));
        assert!(!values.contains(&"opus"));
        assert!(values.contains(&"--model"));
        assert!(values.contains(&"--cwd"));
        assert!(values.contains(&"--"));
    }

    #[test]
    fn completes_setup_subcommands() {
        let candidates = super::complete_with_provider_summaries(
            &["setup".to_string(), "".to_string()],
            &providers(),
        )
        .expect("complete setup");
        let values = candidates
            .iter()
            .map(|candidate| candidate.value.as_str())
            .collect::<Vec<_>>();

        assert_eq!(values, vec!["shell", "alias"]);
    }

    #[test]
    fn generates_zsh_script_that_calls_complete_endpoint() {
        let script = super::generate_completion_script(
            super::ShellKind::Zsh,
            "cc-switch",
            "/usr/local/bin/cc-switch",
        );

        assert!(script.contains("/usr/local/bin/cc-switch __complete"));
        assert!(script.contains("compdef _cc_switch cc-switch"));
    }

    #[test]
    fn generates_bash_script_that_calls_complete_endpoint() {
        let script = super::generate_completion_script(
            super::ShellKind::Bash,
            "cc-switch",
            "/usr/local/bin/cc-switch",
        );

        assert!(script.contains("/usr/local/bin/cc-switch __complete"));
        assert!(script.contains("complete -F _cc_switch cc-switch"));
    }

    #[test]
    fn marker_block_insert_and_replace_are_idempotent() {
        let first = super::upsert_marker_block("export PATH=/bin\n", "completion", "eval one\n");
        let second = super::upsert_marker_block(&first, "completion", "eval two\n");

        assert!(second.contains("export PATH=/bin"));
        assert!(second.contains("eval two"));
        assert!(!second.contains("eval one"));
        assert_eq!(second.matches("# >>> cc-switch completion >>>").count(), 1);
        assert_eq!(second.matches("# <<< cc-switch completion <<<").count(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[serial]
    fn macos_bash_setup_uses_existing_login_profile() {
        let home = tempfile::tempdir().expect("temp home should be created");
        let profile = home.path().join(".profile");
        std::fs::write(&profile, "export PATH=/bin:$PATH\n").expect("profile should be written");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let rc_path = super::default_rc_path(super::ShellKind::Bash).expect("rc path");

        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        assert_eq!(rc_path, profile);
    }

    #[test]
    fn backup_path_does_not_reuse_existing_backup() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let rc_path = dir.path().join(".zshrc");
        let first_backup = super::backup_path_for_timestamp(&rc_path, 123);
        std::fs::write(&first_backup, "old backup").expect("backup placeholder should be written");

        let second_backup = super::backup_path_for_timestamp(&rc_path, 123);

        assert_ne!(second_backup, first_backup);
        assert!(!second_backup.exists());
    }

    #[test]
    fn alias_cc_refuses_existing_command_without_force() {
        let err = super::build_alias_marker_block("cc", "/usr/local/bin/cc-switch", true, false)
            .expect_err("refuse existing cc");

        assert!(err.contains("系统编译器"));
        assert!(err.contains("cc-switch setup alias ccs"));
        assert!(err.contains("cc-switch setup alias cc --force"));
    }

    #[test]
    fn alias_cc_allows_force_when_command_exists() {
        let block = super::build_alias_marker_block("cc", "/usr/local/bin/cc-switch", true, true)
            .expect("force alias");

        assert!(block.contains("alias cc=/usr/local/bin/cc-switch"));
    }
}
