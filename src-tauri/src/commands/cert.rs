use std::io::Write;

use tauri::State;

use crate::cert_manager;
use crate::error::AppError;
use crate::store::cert_repo;
use crate::store::models::Certificate;
use crate::store::proxy_repo;
use crate::validators;
use crate::AppState;

#[tauri::command]
pub async fn list_certificates(state: State<'_, AppState>) -> Result<Vec<Certificate>, AppError> {
    let db = state.lock_db()?;
    cert_repo::list_all(&db)
}

#[tauri::command]
pub async fn get_certificate(
    id: String,
    state: State<'_, AppState>,
) -> Result<Certificate, AppError> {
    let db = state.lock_db()?;
    cert_repo::get_by_id(&db, &id)
}

#[tauri::command]
pub async fn generate_self_signed_cert(
    name: String,
    domain: String,
    validity_days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Certificate, AppError> {
    validators::validate_create_cert(&domain)?;

    let days = validity_days.unwrap_or(365);
    let create_cert = cert_manager::generate_self_signed(&state.data_dir, &name, &domain, days)?;
    let db = state.lock_db()?;
    cert_repo::create(&db, &create_cert)
}

#[tauri::command]
pub async fn import_certificate(
    name: String,
    domain: String,
    cert_pem: String,
    key_pem: String,
    expires_at: String,
    state: State<'_, AppState>,
) -> Result<Certificate, AppError> {
    validators::validate_create_cert(&domain)?;

    let create_cert = cert_manager::import_certificate(
        &state.data_dir,
        &name,
        &domain,
        &cert_pem,
        &key_pem,
        &expires_at,
    )?;
    let db = state.lock_db()?;
    cert_repo::create(&db, &create_cert)
}

#[tauri::command]
pub async fn delete_certificate(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.lock_db()?;

    // Check for referencing proxy rules (Fix 2)
    let referencing = proxy_repo::find_by_certificate(&db, &id)?;
    if let Some(rule) = referencing.first() {
        return Err(AppError::Validation(format!(
            "Certificate is in use by rule: {}",
            rule.name
        )));
    }

    // Get the cert to delete its files
    let cert = cert_repo::get_by_id(&db, &id)?;
    cert_repo::delete(&db, &id)?;

    // Clean up certificate files
    let _ = std::fs::remove_file(&cert.cert_path);
    let _ = std::fs::remove_file(&cert.key_path);

    Ok(())
}

#[tauri::command]
pub async fn export_certificate(
    id: String,
    save_path: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.lock_db()?;
    let cert = cert_repo::get_by_id(&db, &id)?;
    drop(db);

    if cert.status != "ready" {
        return Err(AppError::Validation(
            "Only ready certificates can be exported".to_string(),
        ));
    }

    let cert_pem = std::fs::read(&cert.cert_path).map_err(|e| {
        AppError::Certificate(format!("Failed to read certificate file: {}", e))
    })?;
    let key_pem = std::fs::read(&cert.key_path).map_err(|e| {
        AppError::Certificate(format!("Failed to read private key file: {}", e))
    })?;

    // Sanitize domain for filename: replace * with _wildcard
    let safe_domain = cert.domain.replace('*', "_wildcard");

    let file = std::fs::File::create(&save_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(format!("{}.cert.pem", safe_domain), options)
        .map_err(|e| AppError::Certificate(format!("Failed to write zip: {}", e)))?;
    zip.write_all(&cert_pem)
        .map_err(|e| AppError::Certificate(format!("Failed to write zip: {}", e)))?;

    zip.start_file(format!("{}.key.pem", safe_domain), options)
        .map_err(|e| AppError::Certificate(format!("Failed to write zip: {}", e)))?;
    zip.write_all(&key_pem)
        .map_err(|e| AppError::Certificate(format!("Failed to write zip: {}", e)))?;

    zip.finish()
        .map_err(|e| AppError::Certificate(format!("Failed to finalize zip: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub async fn check_expiring_certs(
    within_days: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<Certificate>, AppError> {
    let days = within_days.unwrap_or(30);
    let db = state.lock_db()?;
    cert_repo::get_expiring(&db, days)
}
