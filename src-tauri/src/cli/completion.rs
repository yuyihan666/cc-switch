use super::ParsedCliArgs;
use crate::services::cli_completion::ShellKind;
use crate::services::cli_completion::{
    complete as complete_candidates, generate_completion_script,
};

pub fn print_script(args: ParsedCliArgs) -> Result<i32, String> {
    let shell = args
        .shell
        .as_deref()
        .ok_or_else(|| "用法: cc-switch completion <zsh|bash>".to_string())?;
    let shell = parse_shell(shell)?;
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取当前可执行文件路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    print!("{}", generate_completion_script(shell, "cc-switch", &exe));
    Ok(0)
}

pub fn complete(args: ParsedCliArgs) -> Result<i32, String> {
    let candidates = complete_candidates(&args.completion_tokens)?;
    for candidate in candidates {
        println!("{}\t{}", candidate.value, candidate.description);
    }
    Ok(0)
}

fn parse_shell(shell: &str) -> Result<ShellKind, String> {
    match shell {
        "zsh" => Ok(ShellKind::Zsh),
        "bash" => Ok(ShellKind::Bash),
        other => Err(format!("不支持的 shell: {other}，可用: zsh, bash")),
    }
}
