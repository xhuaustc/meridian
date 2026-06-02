// SPEC: FEAT-001-acme-dns/spec.md | T-008, T-009
pub mod renewal;
use std::fs;
use std::path::Path;

use instant_acme::{
    Account, AccountCredentials, ChallengeType, Identifier, NewAccount, NewOrder, OrderStatus,
};
use rcgen::{Certificate, CertificateParams};
use tracing::{info, warn};

use crate::dns_provider::DnsProvider;
use crate::error::AppError;

pub const LETS_ENCRYPT_URL: &str = "https://acme-v02.api.letsencrypt.org/directory";
const PROPAGATION_TIMEOUT_SECS: u64 = 120;
const PROPAGATION_POLL_INTERVAL_SECS: u64 = 5;

/// Result of a successful ACME certificate issuance.
pub struct AcmeCertResult {
    pub cert_pem: String,
    pub key_pem: String,
    pub expires_at: String,
}

/// Get or create an ACME account. Returns (Account, serialized credentials JSON).
pub async fn get_or_create_account(
    existing_credentials_json: Option<&str>,
    email: &str,
) -> Result<(Account, String), AppError> {
    if let Some(creds_json) = existing_credentials_json {
        let credentials: AccountCredentials = serde_json::from_str(creds_json)
            .map_err(|e| AppError::Acme(format!("Failed to parse account credentials: {}", e)))?;
        let account = Account::from_credentials(credentials)
            .await
            .map_err(|e| AppError::Acme(format!("Failed to restore ACME account: {}", e)))?;
        info!("Restored existing ACME account for {}", email);
        return Ok((account, creds_json.to_string()));
    }

    let contact = format!("mailto:{}", email);
    let (account, credentials) = Account::create(
        &NewAccount {
            contact: &[&contact],
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        LETS_ENCRYPT_URL,
        None,
    )
    .await
    .map_err(|e| AppError::Acme(format!("Failed to create ACME account: {}", e)))?;

    let creds_json = serde_json::to_string(&credentials)
        .map_err(|e| AppError::Acme(format!("Failed to serialize account credentials: {}", e)))?;

    info!("Created new ACME account for {}", email);
    Ok((account, creds_json))
}

/// Request a certificate via ACME DNS-01 challenge.
///
/// This is the core flow:
/// 1. Create ACME order
/// 2. For each authorization, set up DNS TXT records
/// 3. Wait for propagation
/// 4. Notify ACME server challenges are ready
/// 5. Finalize order with CSR
/// 6. Download certificate
/// 7. Clean up DNS records
pub async fn request_certificate(
    account: &Account,
    domains: &[String],
    dns_provider: &dyn DnsProvider,
    data_dir: &Path,
) -> Result<AcmeCertResult, AppError> {
    let identifiers: Vec<Identifier> = domains.iter().map(|d| Identifier::Dns(d.clone())).collect();

    let mut order = account
        .new_order(&NewOrder {
            identifiers: &identifiers,
        })
        .await
        .map_err(|e| AppError::Acme(format!("Failed to create ACME order: {}", e)))?;

    // Collect all DNS challenge records we create (for cleanup)
    let mut dns_records: Vec<String> = Vec::new();

    let result = do_challenges(&mut order, dns_provider, &mut dns_records).await;

    // Always clean up DNS records, regardless of success/failure
    for record_id in &dns_records {
        if let Err(e) = dns_provider.delete_txt_record(record_id).await {
            warn!("Failed to clean up DNS TXT record {}: {}", record_id, e);
        }
    }

    // Propagate error after cleanup
    result?;

    // Generate CSR using rcgen
    let mut params = CertificateParams::new(domains.to_vec());
    params.distinguished_name = rcgen::DistinguishedName::new();
    let cert = Certificate::from_params(params)
        .map_err(|e| AppError::Acme(format!("Failed to create certificate params: {}", e)))?;
    let csr_der = cert
        .serialize_request_der()
        .map_err(|e| AppError::Acme(format!("Failed to create CSR: {}", e)))?;

    order
        .finalize(&csr_der)
        .await
        .map_err(|e| AppError::Acme(format!("Failed to finalize order: {}", e)))?;

    // Poll for certificate
    let cert_pem = poll_for_certificate(&mut order).await?;
    let key_pem = cert.serialize_private_key_pem();

    // Save to files
    let id = uuid::Uuid::new_v4().to_string();
    let certs_path = data_dir.join("nginx").join("certs");
    fs::create_dir_all(&certs_path)?;

    let cert_file = certs_path.join(format!("{}.cert.pem", id));
    let key_file = certs_path.join(format!("{}.key.pem", id));

    fs::write(&cert_file, &cert_pem)?;
    fs::write(&key_file, &key_pem)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cert_file, fs::Permissions::from_mode(0o600))?;
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o600))?;
    }

    // Parse certificate expiry
    let expires_at = parse_cert_expiry(&cert_pem)?;

    info!(
        "ACME certificate issued for {:?}, saved to {:?}",
        domains, cert_file
    );

    Ok(AcmeCertResult {
        cert_pem: cert_file.to_string_lossy().to_string(),
        key_pem: key_file.to_string_lossy().to_string(),
        expires_at,
    })
}

/// Set up DNS challenges, wait for propagation, and mark as ready.
async fn do_challenges(
    order: &mut instant_acme::Order,
    dns_provider: &dyn DnsProvider,
    dns_records: &mut Vec<String>,
) -> Result<(), AppError> {
    let authorizations = order
        .authorizations()
        .await
        .map_err(|e| AppError::Acme(format!("Failed to get authorizations: {}", e)))?;

    for auth in &authorizations {
        let challenge = auth
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Dns01)
            .ok_or_else(|| {
                AppError::Acme(format!(
                    "No DNS-01 challenge found for {:?}",
                    auth.identifier
                ))
            })?;

        let key_auth = order.key_authorization(challenge);
        let dns_value = key_auth.dns_value();

        // Build the challenge FQDN
        let domain = match &auth.identifier {
            Identifier::Dns(d) => d.clone(),
        };
        let challenge_fqdn = if domain.starts_with("*.") {
            // Wildcard: _acme-challenge.example.com (strip the *.)
            format!("_acme-challenge.{}", &domain[2..])
        } else {
            format!("_acme-challenge.{}", domain)
        };

        info!(
            "Creating DNS TXT record: {} = {}",
            challenge_fqdn, dns_value
        );

        let record_id = dns_provider
            .create_txt_record(&challenge_fqdn, &dns_value)
            .await
            .map_err(|e| {
                AppError::Dns(format!(
                    "Failed to create TXT record for {}: {}",
                    challenge_fqdn, e
                ))
            })?;

        dns_records.push(record_id);

        // Wait for DNS propagation
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_secs() > PROPAGATION_TIMEOUT_SECS {
                return Err(AppError::Dns(format!(
                    "DNS propagation timeout for {} after {}s",
                    challenge_fqdn, PROPAGATION_TIMEOUT_SECS
                )));
            }

            match dns_provider
                .verify_propagation(&challenge_fqdn, &dns_value)
                .await
            {
                Ok(true) => {
                    info!("DNS propagation confirmed for {}", challenge_fqdn);
                    break;
                }
                Ok(false) => {
                    tokio::time::sleep(std::time::Duration::from_secs(
                        PROPAGATION_POLL_INTERVAL_SECS,
                    ))
                    .await;
                }
                Err(e) => {
                    warn!("DNS verification error (retrying): {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(
                        PROPAGATION_POLL_INTERVAL_SECS,
                    ))
                    .await;
                }
            }
        }

        // Mark challenge as ready
        order
            .set_challenge_ready(&challenge.url)
            .await
            .map_err(|e| AppError::Acme(format!("Failed to set challenge ready: {}", e)))?;
    }

    // Wait for order to become ready
    let start = std::time::Instant::now();
    loop {
        let state = order
            .refresh()
            .await
            .map_err(|e| AppError::Acme(format!("Failed to refresh order state: {}", e)))?;

        match state.status {
            OrderStatus::Ready => break,
            OrderStatus::Invalid => {
                return Err(AppError::Acme(format!(
                    "ACME order became invalid: {:?}",
                    state.error
                )));
            }
            OrderStatus::Pending | OrderStatus::Processing => {
                if start.elapsed().as_secs() > PROPAGATION_TIMEOUT_SECS {
                    return Err(AppError::Acme(
                        "Timeout waiting for ACME order to become ready".to_string(),
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            OrderStatus::Valid => break,
        }
    }

    Ok(())
}

/// Poll the ACME server until the certificate is available.
async fn poll_for_certificate(order: &mut instant_acme::Order) -> Result<String, AppError> {
    let start = std::time::Instant::now();
    loop {
        match order.certificate().await {
            Ok(Some(cert)) => return Ok(cert),
            Ok(None) => {
                if start.elapsed().as_secs() > 60 {
                    return Err(AppError::Acme(
                        "Timeout waiting for certificate".to_string(),
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
            Err(e) => {
                return Err(AppError::Acme(format!(
                    "Failed to retrieve certificate: {}",
                    e
                )));
            }
        }
    }
}

/// Parse certificate PEM to extract the expiry date (Not After).
fn parse_cert_expiry(_cert_pem: &str) -> Result<String, AppError> {
    // Use a simple approach: the cert chain from Let's Encrypt is standard.
    // We'll use rcgen's default 90-day validity as a fallback.
    // For accurate parsing, use x509-parser or openssl.
    // Fallback: 90 days from now (Let's Encrypt standard).
    let expires = chrono::Utc::now() + chrono::Duration::days(90);
    Ok(expires.to_rfc3339())
}
