use std::collections::HashMap;

use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::request_params;
use crate::store::hostname_settings_repo;
use crate::store::models::{CreateProxyRule, HostnameSettings, ProxyRule, UpdateProxyRule};
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

/// Whether saving a rule would emit the synthesised port-80 HTTP -> HTTPS
/// redirect block. The block has no row behind it, so the form cannot derive
/// this from the rule list.
#[tauri::command]
pub async fn check_force_https(
    domain: String,
    listen_port: u16,
    exclude_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<config_engine::http_config::ForceHttpsCheck, AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_enabled(&db)?;
    Ok(config_engine::http_config::check_force_https(
        &rules,
        &domain,
        listen_port,
        exclude_id.as_deref(),
    ))
}

/// The request-parameter defaults every rule inherits, plus the presets the form
/// can apply in one click. One call, so the form hardcodes neither the global
/// values nor the preset values.
#[tauri::command]
pub fn get_request_param_defaults(
    state: State<'_, AppState>,
) -> Result<request_params::RequestParamCatalog, AppError> {
    let db = state.get_conn()?;
    request_params::catalog(&db)
}

/// Store one global request-parameter default and regenerate the configs, so
/// every rule that inherits it is rewritten while rules carrying their own value
/// stay as they are. Returns the refreshed catalogue, so the settings page and
/// the rule form both show the value now in force.
#[tauri::command]
pub async fn set_request_param_default(
    directive: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<request_params::RequestParamCatalog, AppError> {
    let param = request_params::RequestParam::from_directive(&directive).ok_or_else(|| {
        AppError::Validation(format!(
            "Request parameter '{directive}' is not one of the six request parameters"
        ))
    })?;

    let mut db = state.get_conn()?;
    let tx = db.transaction()?;
    request_params::save_default(&tx, param, &value)?;
    match config_engine::apply_db_state(&tx, &state.data_dir) {
        Ok(_) => {
            if let Err(error) = tx.commit() {
                let _ = config_engine::apply_db_state(&db, &state.data_dir);
                return Err(AppError::Database(error));
            }
        }
        Err(error) => {
            let _ = tx.rollback();
            return Err(error);
        }
    }
    request_params::catalog(&db)
}

/// The HSTS state for a hostname plus the rule whose form owns the toggle.
///
/// HSTS is stored per hostname, so several rules can share one switch. The form
/// asks the backend which rule may present it instead of guessing, which is the
/// same shape as `check_force_https` for the synthesised redirect.
#[tauri::command]
pub async fn get_hostname_hsts(
    domain: String,
    state: State<'_, AppState>,
) -> Result<config_engine::hsts::HstsCheck, AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_enabled(&db)?;
    let settings = hostname_settings_repo::get(&db, domain.trim())?;
    Ok(config_engine::hsts::check(
        &rules,
        settings.as_ref(),
        domain.trim(),
    ))
}

/// Turn HSTS on or off for a hostname and regenerate the configs.
///
/// HSTS is the only hostname setting this ticket writes; the security-header
/// switches keep their stored values.
#[tauri::command]
pub async fn set_hostname_hsts(
    domain: String,
    enabled: bool,
    max_age: i64,
    include_subdomains: bool,
    state: State<'_, AppState>,
) -> Result<HostnameSettings, AppError> {
    validators::validate_hsts_settings(&domain, max_age)?;
    let domain = domain.trim().to_string();
    apply_proxy_change(&state, |db| {
        hostname_settings_repo::set_hsts(db, &domain, enabled, max_age, include_subdomains)
    })
}

/// Hostnames whose HSTS switch is on, for the rule list's certificate warning.
///
/// A certificate that is about to expire on an HSTS-locked hostname is worse
/// than an ordinary expiry: browsers refuse to let the visitor bypass the
/// error, so the site becomes unreachable.
#[tauri::command]
pub async fn list_hsts_hostnames(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let db = state.get_conn()?;
    let settings = hostname_settings_repo::list_all(&db)?;
    Ok(config_engine::hsts::enabled_hostnames(&settings))
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
