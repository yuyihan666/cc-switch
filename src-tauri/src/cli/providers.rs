use super::ParsedCliArgs;
use crate::app_config::AppType;
use crate::database::Database;
use crate::services::cli_launcher::{describe_model_slots, match_provider};

pub fn list(args: ParsedCliArgs) -> Result<i32, String> {
    ensure_claude(args.app.as_deref())?;
    let db = Database::init().map_err(|e| e.to_string())?;
    let providers = db
        .get_all_providers(AppType::Claude.as_str())
        .map_err(|e| e.to_string())?;
    if providers.is_empty() {
        return Err("没有 Claude provider。请先打开 CC Switch 导入或创建 provider。".to_string());
    }
    for provider in providers.values() {
        println!("{}\t{}", provider.id, provider.name);
    }
    Ok(0)
}

pub fn current(args: ParsedCliArgs) -> Result<i32, String> {
    ensure_claude(args.app.as_deref())?;
    let db = Database::init().map_err(|e| e.to_string())?;
    let providers = db
        .get_all_providers(AppType::Claude.as_str())
        .map_err(|e| e.to_string())?;
    let current_id = crate::settings::get_effective_current_provider(&db, &AppType::Claude)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "当前没有 Claude provider。".to_string())?;
    let provider = providers
        .get(&current_id)
        .ok_or_else(|| format!("当前 provider 不存在: {current_id}"))?;
    println!("{}\t{}", provider.id, provider.name);
    Ok(0)
}

pub fn models(args: ParsedCliArgs) -> Result<i32, String> {
    ensure_claude(args.app.as_deref())?;
    let provider_query = args
        .provider
        .ok_or_else(|| "用法: cc-switch models claude <provider>".to_string())?;
    let db = Database::init().map_err(|e| e.to_string())?;
    let providers = db
        .get_all_providers(AppType::Claude.as_str())
        .map_err(|e| e.to_string())?;
    let provider = match_provider(&providers, &provider_query).map_err(|e| e.to_string())?;
    println!("provider\t{}\t{}", provider.id, provider.name);
    println!("slot\tmodel");
    for slot in describe_model_slots(&provider) {
        println!("{}\t{}", slot.name, slot.model.as_deref().unwrap_or("-"));
    }
    Ok(0)
}

fn ensure_claude(app: Option<&str>) -> Result<(), String> {
    match app {
        Some("claude") => Ok(()),
        Some(other) => Err(format!("MVP 阶段只支持 claude，不支持: {other}")),
        None => Err("缺少 app 参数: claude".to_string()),
    }
}
