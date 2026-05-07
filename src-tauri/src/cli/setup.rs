use super::ParsedCliArgs;

pub fn run(args: ParsedCliArgs) -> Result<i32, String> {
    match args.setup_target.as_deref() {
        Some("shell") => {
            let report = crate::services::cli_completion::install_shell_completion()?;
            println!(
                "已安装 {} 补全: {}\n备份: {}",
                report.shell,
                report.rc_path,
                report.backup_path.unwrap_or_else(|| "-".to_string())
            );
            println!("重新加载 shell 后生效，例如: source {}", report.rc_path);
            Ok(0)
        }
        Some("alias") => {
            let alias_name = args
                .alias_name
                .as_deref()
                .ok_or_else(|| "用法: cc-switch setup alias <name> [--force]".to_string())?;
            let report = crate::services::cli_completion::install_alias(alias_name, args.force)?;
            println!(
                "已安装别名 {}: {}\n备份: {}",
                alias_name,
                report.rc_path,
                report.backup_path.unwrap_or_else(|| "-".to_string())
            );
            println!("重新加载 shell 后生效，例如: source {}", report.rc_path);
            Ok(0)
        }
        Some(other) => Err(format!("不支持的 setup 目标: {other}，可用: shell, alias")),
        None => Err("用法: cc-switch setup <shell|alias>".to_string()),
    }
}
