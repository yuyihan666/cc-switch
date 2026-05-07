use super::ParsedCliArgs;

pub fn run(args: ParsedCliArgs) -> Result<i32, String> {
    match args.setup_target.as_deref() {
        Some("shell") => {
            println!("{}", crate::services::cli_completion::doctor_shell()?);
            Ok(0)
        }
        Some(other) => Err(format!("不支持的 doctor 目标: {other}，可用: shell")),
        None => Err("用法: cc-switch doctor shell".to_string()),
    }
}
