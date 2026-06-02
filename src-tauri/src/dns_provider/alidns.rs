// SPEC: FEAT-001-acme-dns/spec.md | T-003
use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::Sha1;

use crate::error::AppError;

use super::DnsProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlidnsCredential {
    pub access_key_id: String,
    pub access_key_secret: String,
}

pub struct AlidnsProvider {
    client: Client,
    access_key_id: String,
    access_key_secret: String,
}

const ALIDNS_API: &str = "https://alidns.aliyuncs.com";

impl AlidnsProvider {
    pub fn new(cred: AlidnsCredential) -> Self {
        Self {
            client: Client::new(),
            access_key_id: cred.access_key_id,
            access_key_secret: cred.access_key_secret,
        }
    }

    /// Build common parameters for Alibaba Cloud API signature v1.
    fn common_params(&self, action: &str) -> Vec<(String, String)> {
        let now = Utc::now();
        vec![
            ("Format".into(), "JSON".into()),
            ("Version".into(), "2015-01-09".into()),
            ("AccessKeyId".into(), self.access_key_id.clone()),
            ("SignatureMethod".into(), "HMAC-SHA1".into()),
            (
                "Timestamp".into(),
                now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            ),
            ("SignatureVersion".into(), "1.0".into()),
            ("SignatureNonce".into(), uuid::Uuid::new_v4().to_string()),
            ("Action".into(), action.into()),
        ]
    }

    /// Sign request parameters and return the full query string.
    fn sign_params(&self, params: &mut Vec<(String, String)>) -> String {
        params.sort_by(|a, b| a.0.cmp(&b.0));

        let canonical: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let string_to_sign = format!("GET&%2F&{}", percent_encode(&canonical));

        let signing_key = format!("{}&", self.access_key_secret);
        let mut mac =
            Hmac::<Sha1>::new_from_slice(signing_key.as_bytes()).expect("HMAC key length");
        mac.update(string_to_sign.as_bytes());
        let signature = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            mac.finalize().into_bytes(),
        );

        params.push(("Signature".into(), signature));

        params
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Find the registered domain for a given FQDN by querying the Alidns domain list.
    /// Returns (domain, rr). For `_acme-challenge.xhua.eu.org` with domain `xhua.eu.org`,
    /// returns `("xhua.eu.org", "_acme-challenge")`.
    async fn find_domain_rr(&self, fqdn: &str) -> Result<(String, String), AppError> {
        let name = fqdn.trim_end_matches('.');

        // Fetch domains from Alidns
        let mut params = self.common_params("DescribeDomains");
        params.push(("PageSize".into(), "100".into()));
        let query = self.sign_params(&mut params);
        let url = format!("{}/?{}", ALIDNS_API, query);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Alidns API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse Alidns response: {}", e)))?;

        let domains: Vec<&str> = body
            .pointer("/Domains/Domain")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| d.get("DomainName").and_then(|n| n.as_str()))
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
                let rr = if name.len() > domain.len() + 1 {
                    &name[..name.len() - domain.len() - 1]
                } else {
                    "@"
                };
                Ok((domain.to_string(), rr.to_string()))
            }
            None => Err(AppError::Dns(format!(
                "No Alidns domain found for '{}'. Available: {:?}",
                fqdn, domains
            ))),
        }
    }
}

fn percent_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
        .replace('*', "%2A")
        .replace("%7E", "~")
}

#[async_trait]
impl DnsProvider for AlidnsProvider {
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, AppError> {
        let (domain, rr) = self.find_domain_rr(fqdn).await?;
        let mut params = self.common_params("AddDomainRecord");
        params.push(("DomainName".into(), domain));
        params.push(("RR".into(), rr));
        params.push(("Type".into(), "TXT".into()));
        params.push(("Value".into(), value.into()));
        params.push(("TTL".into(), "600".into()));

        let query = self.sign_params(&mut params);
        let url = format!("{}/?{}", ALIDNS_API, query);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Alidns API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse Alidns response: {}", e)))?;

        if let Some(record_id) = body.get("RecordId").and_then(|v| v.as_str()) {
            Ok(record_id.to_string())
        } else if let Some(msg) = body.get("Message").and_then(|v| v.as_str()) {
            Err(AppError::Dns(format!("Alidns error: {}", msg)))
        } else {
            Err(AppError::Dns(format!(
                "Unexpected Alidns response: {}",
                body
            )))
        }
    }

    async fn delete_txt_record(&self, record_id: &str) -> Result<(), AppError> {
        let mut params = self.common_params("DeleteDomainRecord");
        params.push(("RecordId".into(), record_id.into()));

        let query = self.sign_params(&mut params);
        let url = format!("{}/?{}", ALIDNS_API, query);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Alidns API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse Alidns response: {}", e)))?;

        if body.get("RecordId").is_some() {
            Ok(())
        } else if let Some(msg) = body.get("Message").and_then(|v| v.as_str()) {
            Err(AppError::Dns(format!("Alidns delete error: {}", msg)))
        } else {
            Ok(()) // Treat as success if no error
        }
    }

    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, AppError> {
        let (domain, rr) = self.find_domain_rr(fqdn).await?;
        let mut params = self.common_params("DescribeDomainRecords");
        params.push(("DomainName".into(), domain));
        params.push(("RRKeyWord".into(), rr));
        params.push(("TypeKeyWord".into(), "TXT".into()));

        let query = self.sign_params(&mut params);
        let url = format!("{}/?{}", ALIDNS_API, query);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Alidns API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse Alidns response: {}", e)))?;

        let records = body
            .pointer("/DomainRecords/Record")
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
        let mut params = self.common_params("DescribeDomains");
        params.push(("PageSize".into(), "1".into()));

        let query = self.sign_params(&mut params);
        let url = format!("{}/?{}", ALIDNS_API, query);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Dns(format!("Alidns API request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to parse Alidns response: {}", e)))?;

        if let Some(total) = body.get("TotalCount").and_then(|v| v.as_u64()) {
            Ok(format!("Connected to Alidns. {} domain(s) found.", total))
        } else if let Some(msg) = body.get("Message").and_then(|v| v.as_str()) {
            Err(AppError::Dns(format!(
                "Alidns authentication failed: {}",
                msg
            )))
        } else {
            Err(AppError::Dns("Unexpected Alidns response".to_string()))
        }
    }
}
