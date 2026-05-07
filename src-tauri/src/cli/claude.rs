use super::ParsedCliArgs;
use crate::services::cli_launcher::{run_claude_with_provider, ClaudeLaunchOptions};

pub fn run(args: ParsedCliArgs) -> Result<i32, String> {
    let provider = args
        .provider
        .ok_or_else(|| "用法: cc-switch claude <provider> [--model <model-id>]".to_string())?;

    run_claude_with_provider(ClaudeLaunchOptions {
        provider,
        explicit_model: args.explicit_model,
        cwd: args.cwd,
        passthrough: args.passthrough,
    })
    .map_err(|e| e.to_string())
}
