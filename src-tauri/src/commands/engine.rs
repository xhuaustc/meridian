use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::nginx_manager;
use crate::store::models::{NginxStatus, PortConflict, ProxyRule};
use crate::store::{access_repo, cert_repo, proxy_repo};
use crate::AppState;

#[tauri::command]
pub async fn get_engine_status(state: State<'_, AppState>) -> Result<NginxStatus, AppError> {
    Ok(nginx_manager::status(&state.data_dir))
}

#[tauri::command]
pub async fn start_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    // Generate configs first
    apply_config_inner(&state)?;
    nginx_manager::start(&state.data_dir)
}

#[tauri::command]
pub async fn stop_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    nginx_manager::stop(&state.data_dir)
}

#[tauri::command]
pub async fn reload_engine(state: State<'_, AppState>) -> Result<Vec<PortConflict>, AppError> {
    let conflicts = apply_config_inner(&state)?;
    nginx_manager::reload(&state.data_dir)?;
    Ok(conflicts)
}

#[tauri::command]
pub async fn restart_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    // Stop nginx (ignore error if not running)
    let _ = nginx_manager::stop(&state.data_dir);
    // Regenerate configs
    apply_config_inner(&state)?;
    // Start nginx
    nginx_manager::start(&state.data_dir)
}

#[tauri::command]
pub async fn apply_config(
    state: State<'_, AppState>,
) -> Result<Vec<PortConflict>, AppError> {
    apply_config_inner(&state)
}

#[tauri::command]
pub async fn test_nginx_config(state: State<'_, AppState>) -> Result<(bool, String), AppError> {
    nginx_manager::test_config(&state.data_dir)
}

#[tauri::command]
pub async fn detect_conflicts(
    state: State<'_, AppState>,
) -> Result<Vec<PortConflict>, AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_enabled(&db)?;
    Ok(config_engine::conflict::detect_conflicts(&rules))
}

#[tauri::command]
pub async fn check_port_conflict(
    state: State<'_, AppState>,
    listen_port: u16,
    proxy_type: String,
    domain: Option<String>,
    path_prefix: Option<String>,
    exclude_id: Option<String>,
) -> Result<Vec<PortConflict>, AppError> {
    let db = state.get_conn()?;
    let mut rules = proxy_repo::list_enabled(&db)?;
    drop(db);

    // Filter out excluded rule
    if let Some(ref eid) = exclude_id {
        rules.retain(|r| r.id != *eid);
    }

    // Create a virtual rule to test against
    let virtual_rule = ProxyRule {
        id: "__virtual__".to_string(),
        name: "__check__".to_string(),
        proxy_type,
        enabled: true,
        listen_port,
        listen_host: "0.0.0.0".to_string(),
        domain,
        path_prefix,
        upstream_host: "127.0.0.1".to_string(),
        upstream_port: 80,
        tls_mode: "none".to_string(),
        certificate_id: None,
        access_list_id: None,
        websocket: false,
        custom_headers: None,
        upstream_targets: None,
        sort_order: 0,
        created_at: String::new(),
        updated_at: String::new(),
    };

    // Add virtual rule and detect conflicts
    rules.push(virtual_rule);
    let all_conflicts = config_engine::conflict::detect_conflicts(&rules);

    // Only return conflicts involving the virtual rule
    let relevant: Vec<PortConflict> = all_conflicts
        .into_iter()
        .filter(|c| c.rule_id == "__virtual__" || c.message.contains("__check__"))
        .collect();

    Ok(relevant)
}

fn apply_config_inner(state: &AppState) -> Result<Vec<PortConflict>, AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_enabled(&db)?;
    let certs = cert_repo::list_all(&db)?;
    let access_lists_raw = access_repo::list_all_lists(&db)?;

    let mut access_lists = Vec::new();
    for al in &access_lists_raw {
        let rules = access_repo::list_rules_by_list(&db, &al.id)?;
        access_lists.push((al.clone(), rules));
    }

    drop(db); // Release the lock before file I/O

    config_engine::generate_all_configs(&state.data_dir, &rules, &certs, &access_lists)
}
