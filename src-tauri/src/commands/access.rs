use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::store::access_repo;
use crate::store::models::{
    AccessList, AccessListDetail, AccessRule, CreateAccessList, CreateAccessRule,
};
use crate::store::{cert_repo, proxy_repo};
use crate::validators;
use crate::AppState;

#[tauri::command]
pub async fn list_access_lists(
    state: State<'_, AppState>,
) -> Result<Vec<AccessListDetail>, AppError> {
    let db = state.get_conn()?;
    let lists = access_repo::list_all_lists(&db)?;
    let mut result = Vec::new();
    for list in lists {
        let rules = access_repo::list_rules_by_list(&db, &list.id)?;
        let bound = proxy_repo::find_by_access_list(&db, &list.id)?;
        let bound_proxies = bound.into_iter().map(|r| r.name).collect();
        result.push(AccessListDetail {
            list,
            rules,
            bound_proxies,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_access_list(
    id: String,
    state: State<'_, AppState>,
) -> Result<AccessListDetail, AppError> {
    let db = state.get_conn()?;
    let with_rules = access_repo::get_list_with_rules(&db, &id)?;
    let bound = proxy_repo::find_by_access_list(&db, &id)?;
    let bound_proxies = bound.into_iter().map(|r| r.name).collect();
    Ok(AccessListDetail {
        list: with_rules.list,
        rules: with_rules.rules,
        bound_proxies,
    })
}

#[tauri::command]
pub async fn create_access_list(
    input: CreateAccessList,
    state: State<'_, AppState>,
) -> Result<AccessList, AppError> {
    validators::validate_create_access_list(&input.name)?;

    let db = state.get_conn()?;

    // Check name uniqueness (case-insensitive)
    if let Some(existing) = access_repo::find_by_name_ci(&db, &input.name)? {
        return Err(AppError::Validation(format!(
            "Access list with name '{}' already exists (id: {})",
            existing.name, existing.id
        )));
    }

    access_repo::create_list(&db, &input)
}

#[tauri::command]
pub async fn update_access_list(
    id: String,
    name: Option<String>,
    default_policy: Option<String>,
    state: State<'_, AppState>,
) -> Result<AccessList, AppError> {
    if let Some(ref n) = name {
        validators::validate_create_access_list(n)?;
    }

    let result = {
        let db = state.get_conn()?;

        // Check name uniqueness if changing name
        if let Some(ref n) = name {
            if let Some(existing) = access_repo::find_by_name_ci(&db, n)? {
                if existing.id != id {
                    return Err(AppError::Validation(format!(
                        "Access list with name '{}' already exists (id: {})",
                        existing.name, existing.id
                    )));
                }
            }
        }

        access_repo::update_list(&db, &id, name.as_deref(), default_policy.as_deref())?
    };

    // Cascade reload
    apply_and_reload_inner(&state)?;

    Ok(result)
}

#[tauri::command]
pub async fn delete_access_list(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let db = state.get_conn()?;

    // Check for referencing proxy rules (Fix 2)
    let referencing = proxy_repo::find_by_access_list(&db, &id)?;
    if let Some(rule) = referencing.first() {
        return Err(AppError::Validation(format!(
            "Access list is in use by rule: {}",
            rule.name
        )));
    }

    access_repo::delete_list(&db, &id)
}

#[tauri::command]
pub async fn create_access_rule(
    input: CreateAccessRule,
    state: State<'_, AppState>,
) -> Result<AccessRule, AppError> {
    validators::validate_ip_cidr(&input.ip_cidr)?;

    let rule = {
        let db = state.get_conn()?;

        // Check for duplicate rule
        if let Some(_existing) = access_repo::find_duplicate_rule(
            &db,
            &input.access_list_id,
            &input.action,
            &input.ip_cidr,
        )? {
            return Err(AppError::Validation(format!(
                "A rule with action '{}' and IP '{}' already exists in this list",
                input.action, input.ip_cidr
            )));
        }

        access_repo::create_rule(&db, &input)?
    };

    // Cascade reload
    apply_and_reload_inner(&state)?;

    Ok(rule)
}

#[tauri::command]
pub async fn delete_access_rule(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    {
        let db = state.get_conn()?;
        access_repo::delete_rule(&db, &id)?;
    }

    // Cascade reload
    apply_and_reload_inner(&state)?;

    Ok(())
}

#[tauri::command]
pub async fn reorder_access_rules(
    access_list_id: String,
    rule_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    {
        let db = state.get_conn()?;
        access_repo::reorder_rules(&db, &access_list_id, &rule_ids)?;
    }

    // Cascade reload
    apply_and_reload_inner(&state)?;

    Ok(())
}

/// Helper: read all data from DB, generate configs, test, and reload.
fn apply_and_reload_inner(state: &AppState) -> Result<(), AppError> {
    let db = state.get_conn()?;
    let rules = proxy_repo::list_enabled(&db)?;
    let certs = cert_repo::list_all(&db)?;
    let access_lists_raw = access_repo::list_all_lists(&db)?;

    let mut access_lists = Vec::new();
    for al in &access_lists_raw {
        let al_rules = access_repo::list_rules_by_list(&db, &al.id)?;
        access_lists.push((al.clone(), al_rules));
    }
    drop(db);

    config_engine::apply_and_reload(&state.data_dir, &rules, &certs, &access_lists)?;
    Ok(())
}
