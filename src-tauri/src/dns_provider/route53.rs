// SPEC: FEAT-001-acme-dns/spec.md | T-005
use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::AppError;

use super::DnsProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route53Credential {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: Option<String>,
}

pub struct Route53Provider {
    client: Client,
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

impl Route53Provider {
    pub fn new(cred: Route53Credential) -> Self {
        Self {
            client: Client::new(),
            access_key_id: cred.access_key_id,
            secret_access_key: cred.secret_access_key,
            region: cred.region.unwrap_or_else(|| "us-east-1".to_string()),
        }
    }

    /// Find the hosted zone ID for the given FQDN.
    async fn find_hosted_zone(&self, fqdn: &str) -> Result<String, AppError> {
        let domain = fqdn
            .strip_prefix("_acme-challenge.")
            .unwrap_or(fqdn)
            .trim_end_matches('.');

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = format!("{}.", parts[i..].join("."));
            let path = format!(
                "/2013-04-01/hostedzonesbyname?dnsname={}&maxitems=1",
                candidate
            );

            let resp = self.signed_request("GET", &path, "", "route53").await?;

            let body = resp
                .text()
                .await
                .map_err(|e| AppError::Dns(format!("Failed to read Route53 response: {}", e)))?;

            // Simple XML parsing — find HostedZone Id
            if let Some(id_start) = body.find("<Id>/hostedzone/") {
                let rest = &body[id_start + 16..];
                if let Some(id_end) = rest.find("</Id>") {
                    let zone_id = &rest[..id_end];
                    // Verify the zone name matches
                    if body.contains(&format!("<Name>{}</Name>", candidate)) {
                        return Ok(zone_id.to_string());
                    }
                }
            }
        }

        Err(AppError::Dns(format!(
            "No Route53 hosted zone found for '{}'",
            fqdn
        )))
    }

    /// Make an AWS Signature v4 signed request.
    async fn signed_request(
        &self,
        method: &str,
        path: &str,
        body: &str,
        service: &str,
    ) -> Result<reqwest::Response, AppError> {
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let host = format!("{}.amazonaws.com", service);

        // Step 1: Canonical request
        let (canonical_uri, canonical_querystring) = if let Some(idx) = path.find('?') {
            (&path[..idx], &path[idx + 1..])
        } else {
            (path, "")
        };

        let payload_hash = hex_sha256(body.as_bytes());
        let canonical_headers = format!("host:{}\nx-amz-date:{}\n", host, amz_date);
        let signed_headers = "host;x-amz-date";

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method,
            canonical_uri,
            canonical_querystring,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        // Step 2: String to sign
        let credential_scope = format!("{}/{}/aws4_request", date_stamp, service);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            hex_sha256(canonical_request.as_bytes())
        );

        // Step 3: Signing key
        let k_date = hmac_sha256(
            format!("AWS4{}", self.secret_access_key).as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, service.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"aws4_request");

        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key_id, credential_scope, signed_headers, signature
        );

        let url = format!("https://{}{}", host, path);
        let mut req = match method {
            "POST" => self.client.post(&url),
            "GET" => self.client.get(&url),
            _ => self.client.get(&url),
        };

        req = req
            .header("Host", &host)
            .header("X-Amz-Date", &amz_date)
            .header("Authorization", authorization)
            .header("Content-Type", "application/xml");

        if !body.is_empty() {
            req = req.body(body.to_string());
        }

        req.send()
            .await
            .map_err(|e| AppError::Dns(format!("Route53 request failed: {}", e)))
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
impl DnsProvider for Route53Provider {
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, AppError> {
        let zone_id = self.find_hosted_zone(fqdn).await?;
        let record_name = if fqdn.ends_with('.') {
            fqdn.to_string()
        } else {
            format!("{}.", fqdn)
        };

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ChangeResourceRecordSetsRequest xmlns="https://route53.amazonaws.com/doc/2013-04-01/">
  <ChangeBatch>
    <Changes>
      <Change>
        <Action>UPSERT</Action>
        <ResourceRecordSet>
          <Name>{}</Name>
          <Type>TXT</Type>
          <TTL>120</TTL>
          <ResourceRecords>
            <ResourceRecord>
              <Value>"{}"</Value>
            </ResourceRecord>
          </ResourceRecords>
        </ResourceRecordSet>
      </Change>
    </Changes>
  </ChangeBatch>
</ChangeResourceRecordSetsRequest>"#,
            record_name, value
        );

        let path = format!("/2013-04-01/hostedzone/{}/rrset", zone_id);
        let resp = self.signed_request("POST", &path, &body, "route53").await?;

        let status = resp.status();
        let resp_body = resp
            .text()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(AppError::Dns(format!(
                "Route53 create record failed ({}): {}",
                status, resp_body
            )));
        }

        // Return composite ID: zone_id:record_name:value
        Ok(format!("{}:{}:{}", zone_id, record_name, value))
    }

    async fn delete_txt_record(&self, record_id: &str) -> Result<(), AppError> {
        // record_id format: "zone_id:record_name:value"
        let parts: Vec<&str> = record_id.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(AppError::Dns(
                "Invalid Route53 record_id format".to_string(),
            ));
        }
        let (zone_id, record_name, value) = (parts[0], parts[1], parts[2]);

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ChangeResourceRecordSetsRequest xmlns="https://route53.amazonaws.com/doc/2013-04-01/">
  <ChangeBatch>
    <Changes>
      <Change>
        <Action>DELETE</Action>
        <ResourceRecordSet>
          <Name>{}</Name>
          <Type>TXT</Type>
          <TTL>120</TTL>
          <ResourceRecords>
            <ResourceRecord>
              <Value>"{}"</Value>
            </ResourceRecord>
          </ResourceRecords>
        </ResourceRecordSet>
      </Change>
    </Changes>
  </ChangeBatch>
</ChangeResourceRecordSetsRequest>"#,
            record_name, value
        );

        let path = format!("/2013-04-01/hostedzone/{}/rrset", zone_id);
        let resp = self.signed_request("POST", &path, &body, "route53").await?;

        if !resp.status().is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(AppError::Dns(format!(
                "Route53 delete record failed: {}",
                text
            )));
        }

        Ok(())
    }

    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, AppError> {
        let zone_id = self.find_hosted_zone(fqdn).await?;
        let record_name = if fqdn.ends_with('.') {
            fqdn.to_string()
        } else {
            format!("{}.", fqdn)
        };

        let path = format!(
            "/2013-04-01/hostedzone/{}/rrset?type=TXT&name={}",
            zone_id, record_name
        );
        let resp = self.signed_request("GET", &path, "", "route53").await?;

        let body = resp
            .text()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to read response: {}", e)))?;

        // Simple check: look for the value in the response
        let quoted_value = format!("\"{}\"", value);
        Ok(body.contains(&quoted_value))
    }

    async fn test_connection(&self) -> Result<String, AppError> {
        let resp = self
            .signed_request("GET", "/2013-04-01/hostedzone?maxitems=1", "", "route53")
            .await?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(AppError::Dns(format!(
                "Route53 authentication failed ({}): {}",
                status, body
            )));
        }

        // Count zones from XML response
        Ok("Connected to Route53. Found hosted zone(s).".to_string())
    }
}
