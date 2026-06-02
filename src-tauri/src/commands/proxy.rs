use std::collections::HashMap;

use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::store::models::{
    AccessList, AccessRule, Certificate, CreateProxyRule, ProxyRule, UpdateProxyRule,
};
use crate::store::{access_repo, cert_repo, proxy_repo};
use crate::validators;
use crate::AppState;

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
    let (rules, certs, access_lists) = load_config_data(&tx)?;

    match config_engine::apply_and_reload(&state.data_dir, &rules, &certs, &access_lists) {
        Ok(_) => {
            tx.commit()?;
            Ok(result)
        }
        Err(e) => {
            let _ = tx.rollback();
            Err(e)
        }
    }
}

fn load_config_data(
    db: &rusqlite::Connection,
) -> Result<
    (
        Vec<ProxyRule>,
        Vec<Certificate>,
        Vec<(AccessList, Vec<AccessRule>)>,
    ),
    AppError,
> {
    let rules = proxy_repo::list_enabled(&db)?;
    let certs = cert_repo::list_all(&db)?;
    let access_lists_raw = access_repo::list_all_lists(&db)?;

    let mut access_lists = Vec::new();
    for al in &access_lists_raw {
        let al_rules = access_repo::list_rules_by_list(&db, &al.id)?;
        access_lists.push((al.clone(), al_rules));
    }
    Ok((rules, certs, access_lists))
}
