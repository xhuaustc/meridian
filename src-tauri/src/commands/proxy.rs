use std::collections::HashMap;

use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::store::models::{CreateProxyRule, ProxyRule, UpdateProxyRule};
use crate::store::proxy_repo;
use crate::validators;
use crate::AppState;

#[tauri::command]
pub async fn preview_create_proxy(
    input: CreateProxyRule,
    state: State<'_, AppState>,
) -> Result<config_engine::ConfigPreview, AppError> {
    validators::validate_create_proxy(&input)?;
    let mut db = state.get_conn()?;
    let tx = db.transaction()?;
    proxy_repo::create(&tx, &input)?;
    config_engine::preview_db_state(&tx, &state.data_dir)
}

#[tauri::command]
pub async fn preview_update_proxy(
    id: String,
    input: UpdateProxyRule,
    state: State<'_, AppState>,
) -> Result<config_engine::ConfigPreview, AppError> {
    let mut db = state.get_conn()?;
    let existing = proxy_repo::get_by_id(&db, &id)?;
    validators::validate_update_proxy_merged(&input, &existing)?;
    let tx = db.transaction()?;
    proxy_repo::update(&tx, &id, &input)?;
    config_engine::preview_db_state(&tx, &state.data_dir)
}

/// Response for list_proxies with optional stats.
#[derive(serde::Serialize)]
pub struct ProxyListResponse {
    pub rules: Vec<ProxyRule>,
    pub stats: HashMap<String, i64>,
}

#[tauri::command]
pub async fn list_proxies(
    proxy_type: Option<String>,
    enabled: Option<bool>,
    search: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProxyListResponse, AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_filtered(&db, proxy_type.as_deref(), enabled, search.as_deref())?;
    let stats = proxy_repo::count_by_type(&db)?;
    Ok(ProxyListResponse { rules, stats })
}

#[tauri::command]
pub async fn get_proxy(id: String, state: State<'_, AppState>) -> Result<ProxyRule, AppError> {
    let db = state.get_conn()?;
    proxy_repo::get_by_id(&db, &id)
}

#[tauri::command]
pub async fn create_proxy(
    input: CreateProxyRule,
    state: State<'_, AppState>,
) -> Result<ProxyRule, AppError> {
    validators::validate_create_proxy(&input)?;
    apply_proxy_change(&state, |db| proxy_repo::create(db, &input))
}

#[tauri::command]
pub async fn update_proxy(
    id: String,
    input: UpdateProxyRule,
    state: State<'_, AppState>,
) -> Result<ProxyRule, AppError> {
    // Load existing rule first so we can do merged cross-field validation
    let existing = {
        let db = state.get_conn()?;
        proxy_repo::get_by_id(&db, &id)?
    };
    validators::validate_update_proxy_merged(&input, &existing)?;

    apply_proxy_change(&state, |db| proxy_repo::update(db, &id, &input))
}

#[tauri::command]
pub async fn delete_proxy(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    apply_proxy_change(&state, |db| proxy_repo::delete(db, &id))
}

#[tauri::command]
pub async fn toggle_proxy(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<ProxyRule, AppError> {
    apply_proxy_change(&state, |db| proxy_repo::toggle_enabled(db, &id, enabled))
}

#[tauri::command]
pub async fn batch_toggle_proxies(
    ids: Vec<String>,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<usize, AppError> {
    apply_proxy_change(&state, |db| proxy_repo::batch_toggle(db, &ids, enabled))
}

#[tauri::command]
pub async fn batch_delete_proxies(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize, AppError> {
    apply_proxy_change(&state, |db| proxy_repo::batch_delete(db, &ids))
}

fn apply_proxy_change<T, F>(state: &AppState, change: F) -> Result<T, AppError>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, AppError>,
{
    let mut db = state.get_conn()?;
    let tx = db.transaction()?;
    let result = change(&tx)?;
    match config_engine::apply_db_state(&tx, &state.data_dir) {
        Ok(_) => {
            if let Err(error) = tx.commit() {
                let _ = config_engine::apply_db_state(&db, &state.data_dir);
                return Err(AppError::Database(error));
            }
            Ok(result)
        }
        Err(e) => {
            let _ = tx.rollback();
            Err(e)
        }
    }
}
