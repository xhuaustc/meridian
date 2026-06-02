// SPEC: FEAT-001-acme-dns/spec.md | T-004
use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::AppError;

use super::DnsProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnspodCredential {
    pub secret_id: String,
    pub secret_key: String,
}

pub struct DnspodProvider {
    client: Client,
    secret_id: String,
    secret_key: String,
}

const DNSPOD_API: &str = "https://dnspod.tencentcloudapi.com";

impl DnspodProvider {
    pub fn new(cred: DnspodCredential) -> Self {
        Self {
            client: Client::new(),
            secret_id: cred.secret_id,
            secret_key: cred.secret_key,
        }
    }

    /// Build Tencent Cloud API v3 signature headers.
    fn build_request(
        &self,
        action: &str,
        payload: &serde_json::Value,
    ) -> Result<reqwest::RequestBuilder, AppError> {
        let now = Utc::now();
        let timestamp = now.timestamp().to_string();
        let date = now.format("%Y-%m-%d").to_string();

        let payload_str = serde_json::to_string(payload)
            .map_err(|e| AppError::Dns(format!("Failed to serialize payload: {}", e)))?;

        // Step 1: Build canonical request
        let hashed_payload = hex_sha256(payload_str.as_bytes());
        let canonical_request = format!(
            "POST\n/\n\ncontent-type:application/json\nhost:dnspod.tencentcloudapi.com\n\ncontent-type;host\n{}",
            hashed_payload
        );

        // Step 2: Build string to sign
        let credential_scope = format!("{}/dnspod/tc3_request", date);
        let hashed_canonical = hex_sha256(canonical_request.as_bytes());
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{}\n{}\n{}",
            timestamp, credential_scope, hashed_canonical
        );

        // Step 3: Calculate signature
        let secret_date = hmac_sha256(
            format!("TC3{}", self.secret_key).as_bytes(),
            date.as_bytes(),
        );
        let secret_service = hmac_sha256(&secret_date, b"dnspod");
        let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
        let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{}, SignedHeaders=content-type;host, Signature={}",
            self.secret_id, credential_scope, signature
        );

        Ok(self
            .client
            .post(DNSPOD_API)
            .header("Content-Type", "application/json")
            .header("Host", "dnspod.tencentcloudapi.com")
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", &timestamp)
            .header("X-TC-Version", "2021-03-23")
            .header("Authorization", authorization)
            .json(payload))
    }

    /// Find the registered domain for a given FQDN by querying the DNSPod domain list.
    /// Returns (domain, subdomain). For `_acme-challenge.xhua.eu.org` with domain `xhua.eu.org`,
    /// returns `("xhua.eu.org", "_acme-challenge")`.
    async fn find_domain_sub(&self, fqdn: &str) -> Result<(String, String), AppError> {
        let name = fqdn.trim_end_matches('.');

        // Fetch all domains from DNSPod
        let payload = serde_json::json!({ "Limit": 3000 });
        let req = self.build_request("DescribeDomainList", &payload)?;
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("DNSPod API request failed: {}", e)))?;
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse DNSPod response: {}", e)))?;

        let domains: Vec<&str> = body
            .pointer("/Response/DomainList")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| d.get("Name").and_then(|n| n.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        // Find the longest matching domain (most specific)
        let mut best: Option<&str> = None;
        for d in &domains {
            if name == *d || name.ends_with(&format!(".{}", d)) {
                if best.map_or(true, |b| d.len() > b.len()) {
                    best = Some(d);
                }
            }
        }

        match best {
            Some(domain) => {
                let sub = if name.len() > domain.len() + 1 {
                    &name[..name.len() - domain.len() - 1]
                } else {
                    "@"
                };
                Ok((domain.to_string(), sub.to_string()))
            }
            None => Err(AppError::Dns(format!(
                "No DNSPod domain found for '{}'. Available: {:?}",
                fqdn, domains
            ))),
        }
    }
}

fn hex_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[async_trait]
impl DnsProvider for DnspodProvider {
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, AppError> {
        let (domain, sub) = self.find_domain_sub(fqdn).await?;
        let payload = serde_json::json!({
            "Domain": domain,
            "SubDomain": sub,
            "RecordType": "TXT",
            "RecordLine": "默认",
            "Value": value,
            "TTL": 600
        });

        let req = self.build_request("CreateRecord", &payload)?;
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("DNSPod API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse DNSPod response: {}", e)))?;

        if let Some(record_id) = body.pointer("/Response/RecordId").and_then(|v| v.as_u64()) {
            Ok(format!("{}:{}", domain, record_id))
        } else if let Some(msg) = body
            .pointer("/Response/Error/Message")
            .and_then(|v| v.as_str())
        {
            Err(AppError::Dns(format!("DNSPod error: {}", msg)))
        } else {
            Err(AppError::Dns(format!(
                "Unexpected DNSPod response: {}",
                body
            )))
        }
    }

    async fn delete_txt_record(&self, record_id: &str) -> Result<(), AppError> {
        // record_id format: "domain:record_id"
        let parts: Vec<&str> = record_id.splitn(2, ':').collect();
        let (domain, rec_id) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            return Err(AppError::Dns(
                "Invalid record_id format for DNSPod".to_string(),
            ));
        };

        let rec_id_num: u64 = rec_id
            .parse()
            .map_err(|_| AppError::Dns("Invalid record ID number".to_string()))?;

        let payload = serde_json::json!({
            "Domain": domain,
            "RecordId": rec_id_num
        });

        let req = self.build_request("DeleteRecord", &payload)?;
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("DNSPod API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse DNSPod response: {}", e)))?;

        if body.pointer("/Response/Error").is_some() {
            let msg = body
                .pointer("/Response/Error/Message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Err(AppError::Dns(format!("DNSPod delete error: {}", msg)))
        } else {
            Ok(())
        }
    }

    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, AppError> {
        let (domain, sub) = self.find_domain_sub(fqdn).await?;
        let payload = serde_json::json!({
            "Domain": domain,
            "Subdomain": sub,
            "RecordType": "TXT",
            "Limit": 100
        });

        let req = self.build_request("DescribeRecordList", &payload)?;
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("DNSPod API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse DNSPod response: {}", e)))?;

        let records = body
            .pointer("/Response/RecordList")
            .and_then(|v| v.as_array());

        if let Some(records) = records {
            Ok(records.iter().any(|r| {
                r.get("Value")
                    .and_then(|v| v.as_str())
                    .map_or(false, |v| v == value)
            }))
        } else {
            Ok(false)
        }
    }

    async fn test_connection(&self) -> Result<String, AppError> {
        let payload = serde_json::json!({
            "Limit": 1
        });

        let req = self.build_request("DescribeDomainList", &payload)?;
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("DNSPod API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse DNSPod response: {}", e)))?;

        if let Some(total) = body
            .pointer("/Response/DomainCountInfo/AllTotal")
            .and_then(|v| v.as_u64())
        {
            Ok(format!("Connected to DNSPod. {} domain(s) found.", total))
        } else if let Some(msg) = body
            .pointer("/Response/Error/Message")
            .and_then(|v| v.as_str())
        {
            Err(AppError::Dns(format!(
                "DNSPod authentication failed: {}",
                msg
            )))
        } else {
            Err(AppError::Dns("Unexpected DNSPod response".to_string()))
        }
    }
}
