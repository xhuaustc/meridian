use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRule {
    pub id: String,
    pub name: String,
    pub proxy_type: String,
    pub enabled: bool,
    pub listen_port: u16,
    pub listen_host: String,
    pub domain: Option<String>,
    pub path_prefix: Option<String>,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub tls_mode: String,
    pub certificate_id: Option<String>,
    pub access_list_id: Option<String>,
    pub websocket: bool,
    pub custom_headers: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProxyRule {
    pub name: String,
    pub proxy_type: String,
    pub listen_port: u16,
    pub listen_host: Option<String>,
    pub domain: Option<String>,
    pub path_prefix: Option<String>,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub tls_mode: Option<String>,
    pub certificate_id: Option<String>,
    pub access_list_id: Option<String>,
    pub websocket: Option<bool>,
    pub custom_headers: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProxyRule {
    pub name: Option<String>,
    pub proxy_type: Option<String>,
    pub enabled: Option<bool>,
    pub listen_port: Option<u16>,
    pub listen_host: Option<String>,
    pub domain: Option<String>,
    pub path_prefix: Option<String>,
    pub upstream_host: Option<String>,
    pub upstream_port: Option<u16>,
    pub tls_mode: Option<String>,
    pub certificate_id: Option<String>,
    pub access_list_id: Option<String>,
    pub websocket: Option<bool>,
    pub custom_headers: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub cert_path: String,
    pub key_path: String,
    pub source: String,
    pub expires_at: String,
    pub auto_renew: bool,
    pub created_at: String,
    pub dns_credential_id: Option<String>,
    pub acme_account_id: Option<String>,
    pub acme_domains: Option<String>,
    pub last_renew_error: Option<String>,
    pub last_renew_at: Option<String>,
    pub status: String, // "pending", "ready", "failed"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCertificate {
    pub name: String,
    pub domain: String,
    pub cert_path: String,
    pub key_path: String,
    pub source: String,
    pub expires_at: String,
    pub auto_renew: Option<bool>,
    pub dns_credential_id: Option<String>,
    pub acme_account_id: Option<String>,
    pub acme_domains: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsCredential {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub credentials_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDnsCredential {
    pub name: String,
    pub provider: String,
    pub credentials_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmeAccount {
    pub id: String,
    pub email: String,
    pub account_key_pem: String,
    pub ca_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewalStatus {
    pub cert_id: String,
    pub cert_name: String,
    pub domains: Vec<String>,
    pub expires_at: String,
    pub auto_renew: bool,
    pub last_renew_at: Option<String>,
    pub last_renew_error: Option<String>,
    pub next_renew_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessList {
    pub id: String,
    pub name: String,
    pub default_policy: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccessList {
    pub name: String,
    pub default_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRule {
    pub id: String,
    pub access_list_id: String,
    pub action: String,
    pub ip_cidr: String,
    pub sort_order: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccessRule {
    pub access_list_id: String,
    pub action: String,
    pub ip_cidr: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessListWithRules {
    pub list: AccessList,
    pub rules: Vec<AccessRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSetting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConflict {
    pub rule_id: String,
    pub rule_name: String,
    pub conflict_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxStatus {
    pub status: String, // "running" | "stopped" | "error"
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub error_message: Option<String>,
}

/// Extended access list with rules and bound proxy rule names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessListDetail {
    pub list: AccessList,
    pub rules: Vec<AccessRule>,
    pub bound_proxies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: String,
    pub proxy_rules: Vec<ProxyRule>,
    pub certificates: Vec<Certificate>,
    pub access_lists: Vec<AccessList>,
    pub access_rules: Vec<AccessRule>,
    pub settings: Vec<AppSetting>,
}
