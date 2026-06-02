// SPEC: FEAT-001-acme-dns/spec.md | T-002
pub mod alidns;
pub mod cloudflare;
pub mod dnspod;
pub mod route53;

use async_trait::async_trait;

use crate::error::AppError;

/// Trait for DNS providers that support creating/deleting TXT records for ACME DNS-01 challenges.
#[async_trait]
pub trait DnsProvider: Send + Sync {
    /// Create a TXT record. Returns a record identifier for cleanup.
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, AppError>;
    /// Delete a previously created TXT record.
    async fn delete_txt_record(&self, record_id: &str) -> Result<(), AppError>;
    /// Verify the TXT record is visible (for propagation check).
    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, AppError>;
    /// Test credentials by listing zones. Returns a success message or error.
    async fn test_connection(&self) -> Result<String, AppError>;
}

/// Create a DnsProvider implementation from provider name and credentials JSON.
pub fn create_provider(
    provider: &str,
    credentials_json: &str,
) -> Result<Box<dyn DnsProvider>, AppError> {
    match provider {
        "cloudflare" => {
            let cred: cloudflare::CloudflareCredential = serde_json::from_str(credentials_json)
                .map_err(|e| {
                    AppError::Validation(format!("Invalid Cloudflare credentials: {}", e))
                })?;
            Ok(Box::new(cloudflare::CloudflareProvider::new(cred)))
        }
        "alidns" => {
            let cred: alidns::AlidnsCredential = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid Alidns credentials: {}", e)))?;
            Ok(Box::new(alidns::AlidnsProvider::new(cred)))
        }
        "dnspod" => {
            let cred: dnspod::DnspodCredential = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid DNSPod credentials: {}", e)))?;
            Ok(Box::new(dnspod::DnspodProvider::new(cred)))
        }
        "route53" => {
            let cred: route53::Route53Credential = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid Route53 credentials: {}", e)))?;
            Ok(Box::new(route53::Route53Provider::new(cred)))
        }
        _ => Err(AppError::Validation(format!(
            "Unknown DNS provider: '{}'. Supported: cloudflare, alidns, dnspod, route53",
            provider
        ))),
    }
}

/// Validate that credentials_json matches the expected schema for the given provider.
pub fn validate_credentials(provider: &str, credentials_json: &str) -> Result<(), AppError> {
    match provider {
        "cloudflare" => {
            let _: cloudflare::CloudflareCredential = serde_json::from_str(credentials_json)
                .map_err(|e| {
                    AppError::Validation(format!("Invalid Cloudflare credentials: {}", e))
                })?;
        }
        "alidns" => {
            let v: serde_json::Value = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid JSON: {}", e)))?;
            if v.get("access_key_id").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::Validation(
                    "Alidns credentials require 'access_key_id'".to_string(),
                ));
            }
            if v.get("access_key_secret")
                .and_then(|v| v.as_str())
                .is_none()
            {
                return Err(AppError::Validation(
                    "Alidns credentials require 'access_key_secret'".to_string(),
                ));
            }
        }
        "dnspod" => {
            let v: serde_json::Value = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid JSON: {}", e)))?;
            if v.get("secret_id").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::Validation(
                    "DNSPod credentials require 'secret_id'".to_string(),
                ));
            }
            if v.get("secret_key").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::Validation(
                    "DNSPod credentials require 'secret_key'".to_string(),
                ));
            }
        }
        "route53" => {
            let v: serde_json::Value = serde_json::from_str(credentials_json)
                .map_err(|e| AppError::Validation(format!("Invalid JSON: {}", e)))?;
            if v.get("access_key_id").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::Validation(
                    "Route53 credentials require 'access_key_id'".to_string(),
                ));
            }
            if v.get("secret_access_key")
                .and_then(|v| v.as_str())
                .is_none()
            {
                return Err(AppError::Validation(
                    "Route53 credentials require 'secret_access_key'".to_string(),
                ));
            }
        }
        _ => {
            return Err(AppError::Validation(format!(
                "Unknown DNS provider: '{}'. Supported: cloudflare, alidns, dnspod, route53",
                provider
            )));
        }
    }
    Ok(())
}

/// Mask sensitive values in credentials JSON for display.
/// Replaces secret/token/key values with "****" + last 4 chars.
pub fn mask_credentials(credentials_json: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(credentials_json) else {
        return "{}".to_string();
    };
    if let Some(obj) = v.as_object_mut() {
        for (key, val) in obj.iter_mut() {
            let k = key.to_lowercase();
            if k.contains("secret") || k.contains("token") || k.contains("key") {
                if let Some(s) = val.as_str() {
                    let masked = if s.len() > 4 {
                        format!("****{}", &s[s.len() - 4..])
                    } else {
                        "****".to_string()
                    };
                    *val = serde_json::Value::String(masked);
                }
            }
        }
    }
    serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string())
}
