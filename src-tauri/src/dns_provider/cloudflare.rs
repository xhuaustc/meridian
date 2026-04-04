// SPEC: FEAT-001-acme-dns/spec.md | T-002
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::DnsProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareCredential {
    pub api_token: String,
}

pub struct CloudflareProvider {
    client: Client,
    api_token: String,
}

const CF_API: &str = "https://api.cloudflare.com/client/v4";

impl CloudflareProvider {
    pub fn new(cred: CloudflareCredential) -> Self {
        Self {
            client: Client::new(),
            api_token: cred.api_token,
        }
    }

    /// Find the zone ID for a given FQDN by walking up the domain hierarchy.
    async fn find_zone_id(&self, fqdn: &str) -> Result<String, AppError> {
        // Strip _acme-challenge. prefix if present
        let domain = fqdn
            .strip_prefix("_acme-challenge.")
            .unwrap_or(fqdn)
            .trim_end_matches('.');

        // Try progressively shorter domain suffixes to find the zone
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            let resp = self
                .client
                .get(format!("{}/zones", CF_API))
                .bearer_auth(&self.api_token)
                .query(&[("name", &candidate)])
                .send()
                .await
                .map_err(|e| AppError::Dns(format!("Cloudflare API request failed: {}", e)))?;

            let body: CfResponse<Vec<CfZone>> = resp
                .json()
                .await
                .map_err(|e| AppError::Dns(format!("Failed to parse Cloudflare response: {}", e)))?;

            if let Some(zone) = body.result.into_iter().next() {
                return Ok(zone.id);
            }
        }

        Err(AppError::Dns(format!(
            "No Cloudflare zone found for '{}'",
            fqdn
        )))
    }
}

#[async_trait]
impl DnsProvider for CloudflareProvider {
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, AppError> {
        let zone_id = self.find_zone_id(fqdn).await?;
        let record_name = fqdn.trim_end_matches('.');

        let body = serde_json::json!({
            "type": "TXT",
            "name": record_name,
            "content": value,
            "ttl": 120
        });

        let resp = self
            .client
            .post(format!("{}/zones/{}/dns_records", CF_API, zone_id))
            .bearer_auth(&self.api_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to create DNS record: {}", e)))?;

        let status = resp.status();
        let body: CfResponse<CfDnsRecord> = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse response: {}", e)))?;

        if !body.success {
            let errors: Vec<String> = body.errors.iter().map(|e| e.message.clone()).collect();
            return Err(AppError::Dns(format!(
                "Cloudflare API error ({}): {}",
                status,
                errors.join(", ")
            )));
        }

        // Return composite ID so delete_txt_record can find both zone and record
        Ok(format!("{}/{}", zone_id, body.result.id))
    }

    async fn delete_txt_record(&self, record_id: &str) -> Result<(), AppError> {
        // record_id format: "zone_id/record_id"
        let parts: Vec<&str> = record_id.splitn(2, '/').collect();
        let (zone_id, rec_id) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            return Err(AppError::Dns(
                "Invalid record_id format, expected 'zone_id/record_id'".to_string(),
            ));
        };

        let resp = self
            .client
            .delete(format!("{}/zones/{}/dns_records/{}", CF_API, zone_id, rec_id))
            .bearer_auth(&self.api_token)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to delete DNS record: {}", e)))?;

        if !resp.status().is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(AppError::Dns(format!(
                "Failed to delete Cloudflare record: {}",
                text
            )));
        }

        Ok(())
    }

    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, AppError> {
        let zone_id = self.find_zone_id(fqdn).await?;
        let record_name = fqdn.trim_end_matches('.');

        let resp = self
            .client
            .get(format!("{}/zones/{}/dns_records", CF_API, zone_id))
            .bearer_auth(&self.api_token)
            .query(&[("type", "TXT"), ("name", record_name)])
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to query DNS records: {}", e)))?;

        let body: CfResponse<Vec<CfDnsRecord>> = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse response: {}", e)))?;

        Ok(body.result.iter().any(|r| r.content == value))
    }

    async fn test_connection(&self) -> Result<String, AppError> {
        let resp = self
            .client
            .get(format!("{}/zones", CF_API))
            .bearer_auth(&self.api_token)
            .query(&[("per_page", "1")])
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Cloudflare API request failed: {}", e)))?;

        let body: CfResponse<Vec<CfZone>> = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse response: {}", e)))?;

        if !body.success {
            let errors: Vec<String> = body.errors.iter().map(|e| e.message.clone()).collect();
            return Err(AppError::Dns(format!(
                "Cloudflare authentication failed: {}",
                errors.join(", ")
            )));
        }

        Ok(format!(
            "Connected to Cloudflare. {} zone(s) accessible.",
            body.result_info.as_ref().map_or(0, |i| i.total_count)
        ))
    }
}

// --- Cloudflare API response types ---

#[derive(Debug, Deserialize)]
struct CfResponse<T> {
    success: bool,
    result: T,
    #[serde(default)]
    errors: Vec<CfError>,
    result_info: Option<CfResultInfo>,
}

#[derive(Debug, Deserialize)]
struct CfError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct CfResultInfo {
    total_count: u32,
}

#[derive(Debug, Deserialize)]
struct CfZone {
    id: String,
}

#[derive(Debug, Deserialize)]
struct CfDnsRecord {
    id: String,
    #[serde(default)]
    content: String,
}
