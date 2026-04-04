// SPEC: FEAT-001-acme-dns/spec.md | T-010
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

use crate::dns_provider;
use crate::nginx_manager;
use crate::store::cert_repo;
use crate::store::DbPool;

/// Global renewal lock to prevent concurrent renewals.
static RENEWAL_LOCK: std::sync::LazyLock<TokioMutex<()>> =
    std::sync::LazyLock::new(|| TokioMutex::new(()));

/// Run auto-renewal check using a connection from the pool.
pub async fn auto_renew_check(pool: &DbPool, data_dir: &Path) {
    let _guard = match RENEWAL_LOCK.try_lock() {
        Ok(g) => g,
        Err(_) => {
            info!("Auto-renewal already in progress, skipping");
            return;
        }
    };

    info!("Starting auto-renewal check");

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get DB connection for renewal: {}", e);
            return;
        }
    };

    let certs = match cert_repo::list_acme_auto_renew(&conn) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to list ACME certs for renewal: {}", e);
            return;
        }
    };

    // Filter certs expiring within 30 days
    let now = chrono::Utc::now();
    let threshold = now + chrono::Duration::days(30);

    let due: Vec<_> = certs
        .into_iter()
        .filter(|c| {
            chrono::DateTime::parse_from_rfc3339(&c.expires_at)
                .map(|dt| dt < threshold)
                .unwrap_or(true)
        })
        .collect();

    if due.is_empty() {
        info!("No certificates due for renewal");
        return;
    }

    info!("{} certificate(s) due for renewal", due.len());

    for cert in due {
        let domains: Vec<String> = cert
            .acme_domains
            .as_deref()
            .and_then(|d| serde_json::from_str(d).ok())
            .unwrap_or_else(|| vec![cert.domain.clone()]);

        let dns_credential_id = match &cert.dns_credential_id {
            Some(id) => id.clone(),
            None => {
                warn!("Certificate '{}' has no DNS credential, skipping", cert.name);
                continue;
            }
        };

        let acme_account_id = match &cert.acme_account_id {
            Some(id) => id.clone(),
            None => {
                warn!("Certificate '{}' has no ACME account, skipping", cert.name);
                continue;
            }
        };

        info!("Renewing certificate '{}' for {:?}", cert.name, domains);

        // Load data from DB synchronously before doing async work
        let load_result = load_renewal_data(&conn, &dns_credential_id, &acme_account_id);
        let (dns_cred, acme_account) = match load_result {
            Ok(data) => data,
            Err(e) => {
                let err_msg = e.to_string();
                error!("Failed to load renewal data for '{}': {}", cert.name, err_msg);
                let _ = cert_repo::update_cert_after_renewal(
                    &conn, &cert.id, &cert.expires_at, Some(&err_msg),
                );
                continue;
            }
        };

        let result = renew_single_cert(
            data_dir,
            &domains,
            &dns_cred,
            &acme_account,
        )
        .await;

        match result {
            Ok(new_expires) => {
                let _ = cert_repo::update_cert_after_renewal(&conn, &cert.id, &new_expires, None);
                info!("Certificate '{}' renewed, new expiry: {}", cert.name, new_expires);

                if nginx_manager::status(data_dir).status == "running" {
                    let _ = nginx_manager::reload(data_dir);
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                error!("Failed to renew certificate '{}': {}", cert.name, err_msg);

                nginx_manager::append_to_error_log(
                    data_dir,
                    &format!("ACME renewal failed for '{}': {}", cert.name, err_msg),
                );

                let _ = cert_repo::update_cert_after_renewal(
                    &conn,
                    &cert.id,
                    &cert.expires_at,
                    Some(&err_msg),
                );
            }
        }
    }

    info!("Auto-renewal check complete");
}

use crate::store::models::{AcmeAccount, DnsCredential};

/// Load DNS credential and ACME account from DB (sync, before async work).
fn load_renewal_data(
    conn: &Connection,
    dns_credential_id: &str,
    acme_account_id: &str,
) -> Result<(DnsCredential, AcmeAccount), crate::error::AppError> {
    use crate::store::dns_credential_repo;

    let dns_cred = dns_credential_repo::get_by_id(conn, dns_credential_id)?;

    let acme_account: AcmeAccount = {
        let mut stmt = conn.prepare("SELECT * FROM acme_accounts WHERE id = ?1")?;
        stmt.query_row(rusqlite::params![acme_account_id], |row| {
            Ok(AcmeAccount {
                id: row.get("id")?,
                email: row.get("email")?,
                account_key_pem: row.get("account_key_pem")?,
                ca_url: row.get("ca_url")?,
                created_at: row.get("created_at")?,
            })
        })
        .map_err(|_| crate::error::AppError::NotFound("ACME account not found".to_string()))?
    };

    Ok((dns_cred, acme_account))
}

/// Renew a single cert (async — no DB references held).
async fn renew_single_cert(
    data_dir: &Path,
    domains: &[String],
    dns_cred: &DnsCredential,
    acme_account: &AcmeAccount,
) -> Result<String, crate::error::AppError> {
    let provider =
        dns_provider::create_provider(&dns_cred.provider, &dns_cred.credentials_json)?;

    let (account, _) =
        super::get_or_create_account(Some(&acme_account.account_key_pem), &acme_account.email)
            .await?;

    let result =
        super::request_certificate(&account, domains, provider.as_ref(), data_dir).await?;

    Ok(result.expires_at)
}

/// Spawn the auto-renewal background task.
pub fn spawn_renewal_task(pool: DbPool, data_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        // Run initial check after a short delay
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        auto_renew_check(&pool, &data_dir).await;

        // Then run every 12 hours
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(12 * 60 * 60)).await;
            auto_renew_check(&pool, &data_dir).await;
        }
    });
}
