use tauri::State;

use crate::error::AppError;
use crate::hosts_manager;
use crate::store::hosts_repo;
use crate::store::models::{CreateHostEntry, HostEntry};
use crate::validators;
use crate::AppState;

#[tauri::command]
pub async fn list_hosts(
    keyword: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HostEntry>, AppError> {
    let db = state.get_conn()?;
    hosts_repo::list_all(&db, keyword.as_deref())
}

#[tauri::command]
pub async fn create_host(
    input: CreateHostEntry,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    validators::validate_host_entry(&input.ip, &input.hostname)?;

    let entry = {
        let db = state.get_conn()?;

        if let Some(existing) = hosts_repo::find_by_hostname(&db, &input.hostname)? {
            return Err(AppError::Conflict(format!(
                "Hostname '{}' already exists (id: {})",
                existing.hostname, existing.id
            )));
        }

        hosts_repo::create(&db, &input)?
    };

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(entry)
}

#[tauri::command]
pub async fn update_host(
    id: String,
    ip: Option<String>,
    hostname: Option<String>,
    comment: Option<String>,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    if let Some(ref ip) = ip {
        validators::validate_host_ip(ip)?;
    }
    if let Some(ref hn) = hostname {
        validators::validate_hostname(hn)?;
    }

    let entry = {
        let db = state.get_conn()?;

        if let Some(ref hn) = hostname {
            if let Some(existing) = hosts_repo::find_by_hostname(&db, hn)? {
                if existing.id != id {
                    return Err(AppError::Conflict(format!(
                        "Hostname '{}' already exists (id: {})",
                        existing.hostname, existing.id
                    )));
                }
            }
        }

        hosts_repo::update(
            &db,
            &id,
            ip.as_deref(),
            hostname.as_deref(),
            comment.as_deref(),
        )?
    };

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(entry)
}

#[tauri::command]
pub async fn delete_host(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    {
        let db = state.get_conn()?;
        hosts_repo::delete(&db, &id)?;
    }

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_host(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    let entry = {
        let db = state.get_conn()?;
        hosts_repo::toggle(&db, &id, enabled)?
    };

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(entry)
}

#[tauri::command]
pub async fn check_hostname_exists(
    hostname: String,
    exclude_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<HostEntry>, AppError> {
    let db = state.get_conn()?;
    let found = hosts_repo::find_by_hostname(&db, &hostname)?;
    if let Some(ref entry) = found {
        if let Some(ref eid) = exclude_id {
            if &entry.id == eid {
                return Ok(None);
            }
        }
    }
    Ok(found)
}

#[tauri::command]
pub async fn sync_hosts_file(state: State<'_, AppState>) -> Result<(), AppError> {
    sync_hosts_to_system(&state)
}

fn sync_hosts_to_system(state: &AppState) -> Result<(), AppError> {
    let db = state.get_conn()?;
    let entries = hosts_repo::list_enabled(&db)?;
    drop(db);
    hosts_manager::sync_to_system(&entries)
}
