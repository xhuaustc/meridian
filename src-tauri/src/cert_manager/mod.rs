use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use rcgen::{Certificate, CertificateParams, DnType};
use tracing::info;

use crate::error::AppError;
use crate::store::models::CreateCertificate;

/// Directory within data_dir where certificates are stored.
fn certs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("nginx").join("certs")
}

/// Generate a self-signed certificate for the given domain.
/// Returns a CreateCertificate struct ready to be inserted into the database.
pub fn generate_self_signed(
    data_dir: &Path,
    name: &str,
    domain: &str,
    validity_days: u32,
) -> Result<CreateCertificate, AppError> {
    let certs_path = certs_dir(data_dir);
    fs::create_dir_all(&certs_path)?;

    let id = uuid::Uuid::new_v4().to_string();

    let mut params = CertificateParams::new(vec![domain.to_string()]);

    params
        .distinguished_name
        .push(DnType::CommonName, domain);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Meridian Self-Signed");

    let now = Utc::now();
    let not_before = now - Duration::days(1);
    let not_after = now + Duration::days(validity_days as i64);

    params.not_before = rcgen::date_time_ymd(
        not_before.format("%Y").to_string().parse().unwrap(),
        not_before.format("%m").to_string().parse().unwrap(),
        not_before.format("%d").to_string().parse().unwrap(),
    );
    params.not_after = rcgen::date_time_ymd(
        not_after.format("%Y").to_string().parse().unwrap(),
        not_after.format("%m").to_string().parse().unwrap(),
        not_after.format("%d").to_string().parse().unwrap(),
    );

    let cert = Certificate::from_params(params)
        .map_err(|e| AppError::Certificate(format!("Failed to generate certificate: {}", e)))?;

    let cert_pem = cert
        .serialize_pem()
        .map_err(|e| AppError::Certificate(format!("Failed to serialize certificate: {}", e)))?;
    let key_pem = cert.serialize_private_key_pem();

    let cert_file = certs_path.join(format!("{}.cert.pem", id));
    let key_file = certs_path.join(format!("{}.key.pem", id));

    fs::write(&cert_file, &cert_pem)?;
    fs::write(&key_file, &key_pem)?;

    // Set file permissions to 0600 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cert_file, fs::Permissions::from_mode(0o600))?;
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o600))?;
    }

    let expires_at = not_after.to_rfc3339();

    info!(
        "Generated self-signed certificate for '{}' at {:?}",
        domain, cert_file
    );

    Ok(CreateCertificate {
        name: name.to_string(),
        domain: domain.to_string(),
        cert_path: cert_file.to_string_lossy().to_string(),
        key_path: key_file.to_string_lossy().to_string(),
        source: "self_signed".to_string(),
        expires_at,
        auto_renew: Some(false),
    })
}

/// Import a certificate from PEM strings.
/// Validates PEM format, writes files, and returns a CreateCertificate.
pub fn import_certificate(
    data_dir: &Path,
    name: &str,
    domain: &str,
    cert_pem: &str,
    key_pem: &str,
    expires_at: &str,
) -> Result<CreateCertificate, AppError> {
    // Basic PEM validation
    if !cert_pem.contains("-----BEGIN CERTIFICATE-----") {
        return Err(AppError::Certificate(
            "Invalid certificate PEM: missing BEGIN CERTIFICATE marker".to_string(),
        ));
    }
    if !cert_pem.contains("-----END CERTIFICATE-----") {
        return Err(AppError::Certificate(
            "Invalid certificate PEM: missing END CERTIFICATE marker".to_string(),
        ));
    }
    if !key_pem.contains("-----BEGIN") || !key_pem.contains("PRIVATE KEY-----") {
        return Err(AppError::Certificate(
            "Invalid key PEM: missing PRIVATE KEY markers".to_string(),
        ));
    }

    let certs_path = certs_dir(data_dir);
    fs::create_dir_all(&certs_path)?;

    let id = uuid::Uuid::new_v4().to_string();
    let cert_file = certs_path.join(format!("{}.cert.pem", id));
    let key_file = certs_path.join(format!("{}.key.pem", id));

    fs::write(&cert_file, cert_pem)?;
    fs::write(&key_file, key_pem)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cert_file, fs::Permissions::from_mode(0o600))?;
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o600))?;
    }

    info!(
        "Imported certificate for '{}' at {:?}",
        domain, cert_file
    );

    Ok(CreateCertificate {
        name: name.to_string(),
        domain: domain.to_string(),
        cert_path: cert_file.to_string_lossy().to_string(),
        key_path: key_file.to_string_lossy().to_string(),
        source: "upload".to_string(),
        expires_at: expires_at.to_string(),
        auto_renew: Some(false),
    })
}
