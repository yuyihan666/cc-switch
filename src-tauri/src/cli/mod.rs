mod claude;
mod completion;
mod doctor;
mod providers;
mod setup;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Claude,
    Providers,
    Current,
    Models,
    Completion,
    Setup,
    Doctor,
    Complete,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCliArgs {
    pub command: CliCommand,
    pub app: Option<String>,
    pub provider: Option<String>,
    pub explicit_model: Option<String>,
    pub cwd: Option<String>,
    pub passthrough: Vec<String>,
    pub shell: Option<String>,
    pub setup_target: Option<String>,
    pub alias_name: Option<String>,
    pub force: bool,
    pub completion_tokens: Vec<String>,
}

pub fn is_cli_invocation(args: &[String]) -> bool {
    matches!(
        args.get(1).map(String::as_str),
        Some(
            "claude"
                | "providers"
                | "current"
                | "models"
                | "completion"
                | "setup"
                | "doctor"
                | "__complete"
                | "help"
                | "--help"
                | "-h"
        )
    )
}

pub fn parse_cli_args(args: Vec<String>) -> Result<ParsedCliArgs, String> {
    let mut iter = args.into_iter();
    let _program = iter.next();
    let Some(command) = iter.next() else {
        return Ok(help_args());
    };

    match command.as_str() {
        "claude" => parse_claude_args(iter.collect()),
        "providers" => parse_app_query_args(CliCommand::Providers, iter.collect()),
        "current" => parse_app_query_args(CliCommand::Current, iter.collect()),
        "models" => parse_models_args(iter.collect()),
        "completion" => parse_completion_args(iter.collect()),
        "setup" => parse_setup_args(iter.collect()),
        "doctor" => parse_doctor_args(iter.collect()),
        "__complete" => Ok(ParsedCliArgs {
            command: CliCommand::Complete,
            app: None,
            provider: None,
            explicit_model: None,
            cwd: None,
            passthrough: Vec::new(),
            shell: None,
            setup_target: None,
            alias_name: None,
            force: false,
            completion_tokens: iter.collect(),
        }),
        "help" | "--help" | "-h" => Ok(help_args()),
        other => Err(format!("不支持的 CLI 命令: {other}")),
    }
}

pub fn run(args: Vec<String>) -> i32 {
    match parse_cli_args(args).and_then(dispatch) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn dispatch(args: ParsedCliArgs) -> Result<i32, String> {
    match args.command {
        CliCommand::Help => {
            print_help();
            Ok(0)
        }
        CliCommand::Claude => claude::run(args),
        CliCommand::Providers => providers::list(args),
        CliCommand::Current => providers::current(args),
        CliCommand::Models => providers::models(args),
        CliCommand::Completion => completion::print_script(args),
        CliCommand::Setup => setup::run(args),
        CliCommand::Doctor => doctor::run(args),
        CliCommand::Complete => completion::complete(args),
    }
}

fn parse_claude_args(tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let mut provider = None;
    let mut explicit_model = None;
    let mut cwd = None;
    let mut passthrough = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];
        match token.as_str() {
            "--" => {
                passthrough.extend(tokens[i + 1..].iter().cloned());
                break;
            }
            "--model" => {
                i += 1;
                let Some(value) = tokens.get(i) else {
                    return Err("--model 需要模型 ID".to_string());
                };
                explicit_model = Some(value.clone());
            }
            "--cwd" => {
                i += 1;
                let Some(value) = tokens.get(i) else {
                    return Err("--cwd 需要目录路径".to_string());
                };
                cwd = Some(value.clone());
            }
            value if value.starts_with("--") => {
                return Err(format!("不支持的 claude 参数: {value}"));
            }
            value => {
                if provider.is_none() {
                    provider = Some(value.to_string());
                } else {
                    return Err(format!(
                        "多余的位置参数: {value}\n提示: `cc-switch claude <provider>` 会使用 provider 内的默认模型配置；如需强制覆盖模型，使用 `--model <model-id>`"
                    ));
                }
            }
        }
        i += 1;
    }

    if provider.is_none() {
        return Err("用法: cc-switch claude <provider> [--model <model-id>]".to_string());
    }

    Ok(ParsedCliArgs {
        command: CliCommand::Claude,
        app: Some("claude".to_string()),
        provider,
        explicit_model,
        cwd,
        passthrough,
        shell: None,
        setup_target: None,
        alias_name: None,
        force: false,
        completion_tokens: Vec::new(),
    })
}

fn parse_app_query_args(command: CliCommand, tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let app = tokens
        .first()
        .cloned()
        .ok_or_else(|| "查询命令需要 app 参数，例如: cc-switch providers claude".to_string())?;
    if app != "claude" {
        return Err("MVP 阶段 CLI 查询只支持 claude".to_string());
    }

    Ok(ParsedCliArgs {
        command,
        app: Some(app),
        provider: None,
        explicit_model: None,
        cwd: None,
        passthrough: Vec::new(),
        shell: None,
        setup_target: None,
        alias_name: None,
        force: false,
        completion_tokens: Vec::new(),
    })
}

fn parse_models_args(tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let app = tokens
        .first()
        .cloned()
        .ok_or_else(|| "用法: cc-switch models claude <provider>".to_string())?;
    if app != "claude" {
        return Err("MVP 阶段 models 只支持 claude".to_string());
    }
    let provider = tokens
        .get(1)
        .cloned()
        .ok_or_else(|| "用法: cc-switch models claude <provider>".to_string())?;

    Ok(ParsedCliArgs {
        command: CliCommand::Models,
        app: Some(app),
        provider: Some(provider),
        explicit_model: None,
        cwd: None,
        passthrough: Vec::new(),
        shell: None,
        setup_target: None,
        alias_name: None,
        force: false,
        completion_tokens: Vec::new(),
    })
}

fn parse_completion_args(tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let shell = tokens
        .first()
        .cloned()
        .ok_or_else(|| "用法: cc-switch completion <zsh|bash>".to_string())?;
    if !matches!(shell.as_str(), "zsh" | "bash") {
        return Err(format!("不支持的 shell: {shell}，可用: zsh, bash"));
    }
    if tokens.len() > 1 {
        return Err(format!("多余的位置参数: {}", tokens[1]));
    }

    Ok(base_args(CliCommand::Completion).with_shell(shell))
}

fn parse_setup_args(tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let target = tokens
        .first()
        .cloned()
        .ok_or_else(|| "用法: cc-switch setup <shell|alias>".to_string())?;
    match target.as_str() {
        "shell" => {
            if tokens.len() > 1 {
                return Err(format!("多余的位置参数: {}", tokens[1]));
            }
            Ok(base_args(CliCommand::Setup).with_setup_target(target))
        }
        "alias" => {
            let alias_name = tokens
                .get(1)
                .cloned()
                .ok_or_else(|| "用法: cc-switch setup alias <name> [--force]".to_string())?;
            let mut force = false;
            for token in tokens.iter().skip(2) {
                if token == "--force" {
                    force = true;
                } else {
                    return Err(format!("不支持的 setup alias 参数: {token}"));
                }
            }
            let mut args = base_args(CliCommand::Setup).with_setup_target(target);
            args.alias_name = Some(alias_name);
            args.force = force;
            Ok(args)
        }
        other => Err(format!("不支持的 setup 目标: {other}，可用: shell, alias")),
    }
}

fn parse_doctor_args(tokens: Vec<String>) -> Result<ParsedCliArgs, String> {
    let target = tokens
        .first()
        .cloned()
        .ok_or_else(|| "用法: cc-switch doctor shell".to_string())?;
    if target != "shell" {
        return Err(format!("不支持的 doctor 目标: {target}，可用: shell"));
    }
    if tokens.len() > 1 {
        return Err(format!("多余的位置参数: {}", tokens[1]));
    }
    Ok(base_args(CliCommand::Doctor).with_setup_target(target))
}

fn help_args() -> ParsedCliArgs {
    base_args(CliCommand::Help)
}

fn base_args(command: CliCommand) -> ParsedCliArgs {
    ParsedCliArgs {
        command,
        app: None,
        provider: None,
        explicit_model: None,
        cwd: None,
        passthrough: Vec::new(),
        shell: None,
        setup_target: None,
        alias_name: None,
        force: false,
        completion_tokens: Vec::new(),
    }
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    "Usage:
  cc-switch <command> [args]

Commands:
  claude <provider>            用指定供应商启动 Claude Code，模型由 provider 配置决定
  providers claude             查看可用于 Claude 的供应商
  current claude               查看当前 Claude 默认供应商
  models claude <provider>     查看某个供应商的 Claude 模型位
  completion <zsh|bash>        输出 shell 补全脚本
  setup shell                  安装 Tab 补全
  setup alias <name> [--force] 安装短命令别名，例如 ccs
  doctor shell                 检查补全和别名安装状态

Examples:
  cc-switch claude deepseek
  cc-switch claude deepseek -- --version
  cc-switch models claude deepseek
  cc-switch setup shell"
}

impl ParsedCliArgs {
    fn with_shell(mut self, shell: String) -> Self {
        self.shell = Some(shell);
        self
    }

    fn with_setup_target(mut self, target: String) -> Self {
        self.setup_target = Some(target);
        self
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_claude_provider_and_passthrough_args() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "claude".to_string(),
            "deepseek".to_string(),
            "--".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ])
        .expect("parse args");

        assert_eq!(parsed.command, super::CliCommand::Claude);
        assert_eq!(parsed.provider.as_deref(), Some("deepseek"));
        assert_eq!(parsed.passthrough, vec!["--dangerously-skip-permissions"]);
    }

    #[test]
    fn rejects_positional_model_slot_after_provider() {
        let err = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "claude".to_string(),
            "deepseek".to_string(),
            "pro".to_string(),
        ])
        .expect_err("positional model slot should be rejected");

        assert!(err.contains("多余的位置参数: pro"));
        assert!(err.contains("provider 内的默认模型配置"));
        assert!(err.contains("--model <model-id>"));
    }

    #[test]
    fn parses_model_flag() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "claude".to_string(),
            "deepseek".to_string(),
            "--model".to_string(),
            "deepseek-v4-flash".to_string(),
        ])
        .expect("parse args");

        assert_eq!(parsed.command, super::CliCommand::Claude);
        assert_eq!(parsed.provider.as_deref(), Some("deepseek"));
        assert_eq!(parsed.explicit_model.as_deref(), Some("deepseek-v4-flash"));
    }

    #[test]
    fn parses_completion_shell() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "completion".to_string(),
            "zsh".to_string(),
        ])
        .expect("parse completion");

        assert_eq!(parsed.command, super::CliCommand::Completion);
        assert_eq!(parsed.shell.as_deref(), Some("zsh"));
    }

    #[test]
    fn parses_setup_shell() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "setup".to_string(),
            "shell".to_string(),
        ])
        .expect("parse setup shell");

        assert_eq!(parsed.command, super::CliCommand::Setup);
        assert_eq!(parsed.setup_target.as_deref(), Some("shell"));
    }

    #[test]
    fn parses_setup_alias() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "setup".to_string(),
            "alias".to_string(),
            "ccs".to_string(),
        ])
        .expect("parse setup alias");

        assert_eq!(parsed.command, super::CliCommand::Setup);
        assert_eq!(parsed.setup_target.as_deref(), Some("alias"));
        assert_eq!(parsed.alias_name.as_deref(), Some("ccs"));
        assert!(!parsed.force);
    }

    #[test]
    fn parses_setup_alias_force() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "setup".to_string(),
            "alias".to_string(),
            "cc".to_string(),
            "--force".to_string(),
        ])
        .expect("parse setup alias force");

        assert_eq!(parsed.command, super::CliCommand::Setup);
        assert_eq!(parsed.alias_name.as_deref(), Some("cc"));
        assert!(parsed.force);
    }

    #[test]
    fn parses_doctor_shell() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "doctor".to_string(),
            "shell".to_string(),
        ])
        .expect("parse doctor");

        assert_eq!(parsed.command, super::CliCommand::Doctor);
        assert_eq!(parsed.setup_target.as_deref(), Some("shell"));
    }

    #[test]
    fn parses_hidden_complete_tokens() {
        let parsed = super::parse_cli_args(vec![
            "cc-switch".to_string(),
            "__complete".to_string(),
            "claude".to_string(),
            "Deep".to_string(),
        ])
        .expect("parse complete");

        assert_eq!(parsed.command, super::CliCommand::Complete);
        assert_eq!(parsed.completion_tokens, vec!["claude", "Deep"]);
    }

    #[test]
    fn help_text_explains_commands_and_examples() {
        let help = super::help_text();

        assert!(help.contains("Commands:"));
        assert!(help.contains("Examples:"));
        assert!(help.contains("用指定供应商启动 Claude Code"));
        assert!(help.contains("模型由 provider 配置决定"));
        assert!(help.contains("安装短命令别名"));
        assert!(!help.contains("[profile]"));
    }
}
