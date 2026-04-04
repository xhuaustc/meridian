use tauri::State;

use crate::error::AppError;
use crate::store::models::{AppSetting, ExportData};
use crate::store::{access_repo, cert_repo, proxy_repo, settings_repo};
use crate::AppState;

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let db = state.lock_db()?;
    settings_repo::get(&db, &key)
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.lock_db()?;
    settings_repo::set(&db, &key, &value)
}

#[tauri::command]
pub async fn list_settings(state: State<'_, AppState>) -> Result<Vec<AppSetting>, AppError> {
    let db = state.lock_db()?;
    settings_repo::list_all(&db)
}

#[tauri::command]
pub async fn export_data(state: State<'_, AppState>) -> Result<ExportData, AppError> {
    let db = state.lock_db()?;
    let proxy_rules = proxy_repo::list_all(&db)?;
    let certificates = cert_repo::list_all(&db)?;
    let access_lists = access_repo::list_all_lists(&db)?;
    let access_rules = access_repo::list_all_rules(&db)?;
    let settings = settings_repo::list_all(&db)?;

    Ok(ExportData {
        version: "1.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        proxy_rules,
        certificates,
        access_lists,
        access_rules,
        settings,
    })
}

#[tauri::command]
pub async fn import_data(
    data: ExportData,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.lock_db()?;

    // Import in a transaction
    db.execute_batch("BEGIN TRANSACTION;")?;

    let result = (|| -> Result<(), AppError> {
        // Clear existing data
        db.execute_batch(
            "DELETE FROM access_rules;
             DELETE FROM proxy_rules;
             DELETE FROM certificates;
             DELETE FROM access_lists;
             DELETE FROM app_settings;",
        )?;

        // Import settings
        for setting in &data.settings {
            settings_repo::set(&db, &setting.key, &setting.value)?;
        }

        // Import certificates
        for cert in &data.certificates {
            db.execute(
                "INSERT INTO certificates (id, name, domain, cert_path, key_path, source, expires_at, auto_renew, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    cert.id,
                    cert.name,
                    cert.domain,
                    cert.cert_path,
                    cert.key_path,
                    cert.source,
                    cert.expires_at,
                    if cert.auto_renew { 1 } else { 0 },
                    cert.created_at,
                ],
            )?;
        }

        // Import access lists
        for al in &data.access_lists {
            db.execute(
                "INSERT INTO access_lists (id, name, default_policy, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![al.id, al.name, al.default_policy, al.created_at],
            )?;
        }

        // Import access rules
        for ar in &data.access_rules {
            db.execute(
                "INSERT INTO access_rules (id, access_list_id, action, ip_cidr, sort_order, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    ar.id,
                    ar.access_list_id,
                    ar.action,
                    ar.ip_cidr,
                    ar.sort_order,
                    ar.created_at,
                ],
            )?;
        }

        // Import proxy rules
        for pr in &data.proxy_rules {
            db.execute(
                "INSERT INTO proxy_rules (id, name, proxy_type, enabled, listen_port, listen_host, domain, path_prefix, upstream_host, upstream_port, tls_mode, certificate_id, access_list_id, websocket, custom_headers, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                rusqlite::params![
                    pr.id,
                    pr.name,
                    pr.proxy_type,
                    if pr.enabled { 1 } else { 0 },
                    pr.listen_port as u32,
                    pr.listen_host,
                    pr.domain,
                    pr.path_prefix,
                    pr.upstream_host,
                    pr.upstream_port as u32,
                    pr.tls_mode,
                    pr.certificate_id,
                    pr.access_list_id,
                    if pr.websocket { 1 } else { 0 },
                    pr.custom_headers,
                    pr.sort_order,
                    pr.created_at,
                    pr.updated_at,
                ],
            )?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(e) => {
            let _ = db.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>) -> Result<String, AppError> {
    let db_path = state.data_dir.join("meridian.db");
    crate::store::backup_database(&db_path)
}
