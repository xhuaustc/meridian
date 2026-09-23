use tauri::State;

use crate::config_engine;
use crate::error::AppError;
use crate::recovery::{self, RecoveryPreview};
use crate::store::models::AppSetting;
use crate::store::settings_repo;
use crate::AppState;

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let db = state.get_conn()?;
    settings_repo::get(&db, &key)
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let mut db = state.get_conn()?;
    if key == "worker_processes" {
        if value != "auto" && !value.parse::<u8>().is_ok_and(|n| (1..=64).contains(&n)) {
            return Err(AppError::Validation(
                "Worker processes must be auto or 1-64".into(),
            ));
        }
        let tx = db.transaction()?;
        settings_repo::set(&tx, &key, &value)?;
        config_engine::apply_db_state(&tx, &state.data_dir)?;
        if let Err(error) = tx.commit() {
            let _ = config_engine::apply_db_state(&db, &state.data_dir);
            return Err(AppError::Database(error));
        }
        return Ok(());
    }
    settings_repo::set(&db, &key, &value)
}

#[tauri::command]
pub async fn list_settings(state: State<'_, AppState>) -> Result<Vec<AppSetting>, AppError> {
    let db = state.get_conn()?;
    settings_repo::list_all(&db)
}

#[tauri::command]
pub async fn create_recovery_bundle(
    save_path: String,
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<RecoveryPreview, AppError> {
    let db_path = state.data_dir.join("meridian.db");
    recovery::create_bundle(&db_path, std::path::Path::new(&save_path), &passphrase)
}

#[tauri::command]
pub async fn preview_recovery_bundle(
    file_path: String,
    passphrase: String,
) -> Result<RecoveryPreview, AppError> {
    recovery::preview_bundle(std::path::Path::new(&file_path), &passphrase)
}

#[tauri::command]
pub async fn restore_recovery_bundle(
    file_path: String,
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let db_path = state.data_dir.join("meridian.db");
    let mut db = state.get_conn()?;
    recovery::restore_bundle(
        &mut db,
        &db_path,
        &state.data_dir,
        std::path::Path::new(&file_path),
        &passphrase,
    )
}
