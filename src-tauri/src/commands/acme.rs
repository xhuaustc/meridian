// SPEC: FEAT-001-acme-dns/spec.md | T-009, T-011
use std::path::PathBuf;

use tauri::State;
use tracing::{error, info};

use crate::acme_client;
use crate::dns_provider;
use crate::error::AppError;
use crate::nginx_manager;
use crate::store::{acme_repo, cert_repo, dns_credential_repo};
use crate::store::models::{Certificate, RenewalStatus};
use crate::AppState;

#[tauri::command]
pub async fn request_acme_cert(
    domains: Vec<String>,
    dns_credential_id: String,
    email: String,
    auto_renew: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Certificate, AppError> {
    if domains.is_empty() {
        return Err(AppError::Validation("At least one domain is required".to_string()));
    }
    if email.trim().is_empty() {
        return Err(AppError::Validation("Email is required".to_string()));
    }

    // Validate DNS credential exists (fast, local DB only)
    let dns_cred = {
        let db = state.get_conn()?;
        dns_credential_repo::get_by_id(&db, &dns_credential_id)?
    };

    // Build display name
    let primary_domain = domains
        .iter()
        .find(|d| !d.starts_with("*."))
        .unwrap_or(&domains[0])
        .clone();

    let cert_name = if domains.len() == 1 {
        domains[0].clone()
    } else {
        format!("{} (+{})", primary_domain, domains.len() - 1)
    };

    let domains_json = serde_json::to_string(&domains)
        .map_err(|e| AppError::Acme(format!("Failed to serialize domains: {}", e)))?;

    let auto = auto_renew.unwrap_or(true);

    // Create a pending certificate record and return immediately — no network calls
    let cert = {
        let db = state.get_conn()?;
        cert_repo::create_pending(
            &db,
            &cert_name,
            &primary_domain,
            auto,
            &dns_credential_id,
            &domains_json,
        )?
    };

    // Spawn background task for the full ACME flow (including account creation)
    let cert_id = cert.id.clone();
    let data_dir = state.data_dir.clone();
    let db_path = state.data_dir.join("meridian.db");

    tauri::async_runtime::spawn(async move {
        let result = do_acme_background(
            &db_path,
            &data_dir,
            &cert_id,
            &domains,
            &email,
            dns_cred.provider,
            dns_cred.credentials_json,
        )
        .await;

        if let Err(e) = result {
            error!("Background ACME request failed for cert {}: {}", cert_id, e);
        }
    });

    Ok(cert)
}

/// Run the full ACME flow in the background and update the cert record.
async fn do_acme_background(
    db_path: &PathBuf,
    data_dir: &PathBuf,
    cert_id: &str,
    domains: &[String],
    email: &str,
    provider_name: String,
    credentials_json: String,
) -> Result<(), AppError> {
    let provider = dns_provider::create_provider(&provider_name, &credentials_json)?;

    // Get or create ACME account (this is the slow network call)
    let existing_key = {
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| AppError::Database(e))?;
        acme_repo::find_by_email(&conn, email)?
            .map(|a| a.account_key_pem)
    };

    let (account, creds_json) =
        acme_client::get_or_create_account(existing_key.as_deref(), email).await?;

    // Save ACME account if new, and update the cert's acme_account_id
    {
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| AppError::Database(e))?;
        if existing_key.is_none() {
            acme_repo::create(&conn, email, &creds_json, acme_client::LETS_ENCRYPT_URL)?;
        }
        let acme_account_id = acme_repo::find_by_email(&conn, email)?
            .map(|a| a.id)
            .unwrap_or_default();
        conn.execute(
            "UPDATE certificates SET acme_account_id = ?1 WHERE id = ?2",
            rusqlite::params![acme_account_id, cert_id],
        ).map_err(|e| AppError::Database(e))?;
    }

    // Do the actual certificate request
    match acme_client::request_certificate(&account, domains, provider.as_ref(), data_dir).await {
        Ok(result) => {
            let conn = rusqlite::Connection::open(db_path)
                .map_err(|e| AppError::Database(e))?;
            cert_repo::finish_pending(
                &conn,
                cert_id,
                &result.cert_pem,
                &result.key_pem,
                &result.expires_at,
            )?;
            info!("ACME certificate {} issued successfully", cert_id);

            // Reload nginx if running
            if nginx_manager::status(data_dir).status == "running" {
                let _ = nginx_manager::reload(data_dir);
            }
        }
        Err(e) => {
            let err_msg = e.to_string();
            error!("ACME certificate {} failed: {}", cert_id, err_msg);
            let conn = rusqlite::Connection::open(db_path)
                .map_err(|e| AppError::Database(e))?;
            cert_repo::fail_pending(&conn, cert_id, &err_msg)?;

            nginx_manager::append_to_error_log(
                data_dir,
                &format!("ACME request failed for cert {}: {}", cert_id, err_msg),
            );
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_acme_renewal_status(
    state: State<'_, AppState>,
) -> Result<Vec<RenewalStatus>, AppError> {
    let db = state.get_conn()?;
    let certs = cert_repo::list_acme_auto_renew(&db)?;

    let statuses: Vec<RenewalStatus> = certs
        .into_iter()
        .map(|cert| {
            let domains: Vec<String> = cert
                .acme_domains
                .as_deref()
                .and_then(|d| serde_json::from_str(d).ok())
                .unwrap_or_else(|| vec![cert.domain.clone()]);

            // next_renew_at = expires_at - 30 days
            let next_renew_at = chrono::DateTime::parse_from_rfc3339(&cert.expires_at)
                .map(|dt| (dt - chrono::Duration::days(30)).to_rfc3339())
                .unwrap_or_else(|_| cert.expires_at.clone());

            RenewalStatus {
                cert_id: cert.id,
                cert_name: cert.name,
                domains,
                expires_at: cert.expires_at,
                auto_renew: cert.auto_renew,
                last_renew_at: cert.last_renew_at,
                last_renew_error: cert.last_renew_error,
                next_renew_at,
            }
        })
        .collect();

    Ok(statuses)
}
