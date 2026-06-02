// SPEC: FEAT-001-acme-dns/spec.md | T-006, T-007
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::dns_provider;
use crate::error::AppError;
use crate::store::models::{CreateDnsCredential, DnsCredential};
use crate::store::{cert_repo, dns_credential_repo};
use crate::AppState;

/// DnsCredential with masked credentials for list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsCredentialMasked {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub credentials_json: String,
    pub created_at: String,
    pub updated_at: String,
}

impl DnsCredentialMasked {
    fn from(cred: DnsCredential) -> Self {
        Self {
            id: cred.id,
            name: cred.name,
            provider: cred.provider,
            credentials_json: dns_provider::mask_credentials(&cred.credentials_json),
            created_at: cred.created_at,
            updated_at: cred.updated_at,
        }
    }
}

#[tauri::command]
pub async fn list_dns_credentials(
    state: State<'_, AppState>,
) -> Result<Vec<DnsCredentialMasked>, AppError> {
    let db = state.get_conn()?;
    let creds = dns_credential_repo::list_all(&db)?;
    Ok(creds.into_iter().map(DnsCredentialMasked::from).collect())
}

#[tauri::command]
pub async fn create_dns_credential(
    name: String,
    provider: String,
    credentials_json: String,
    state: State<'_, AppState>,
) -> Result<DnsCredential, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Validation("Name cannot be empty".to_string()));
    }

    // Validate provider and credentials schema
    dns_provider::validate_credentials(&provider, &credentials_json)?;

    let input = CreateDnsCredential {
        name,
        provider,
        credentials_json,
    };

    let db = state.get_conn()?;
    dns_credential_repo::create(&db, &input)
}

#[tauri::command]
pub async fn update_dns_credential(
    id: String,
    name: Option<String>,
    credentials_json: Option<String>,
    state: State<'_, AppState>,
) -> Result<DnsCredential, AppError> {
    if let Some(ref n) = name {
        if n.trim().is_empty() {
            return Err(AppError::Validation("Name cannot be empty".to_string()));
        }
    }

    // If updating credentials, validate against the existing provider
    if let Some(ref cj) = credentials_json {
        let db = state.get_conn()?;
        let existing = dns_credential_repo::get_by_id(&db, &id)?;
        dns_provider::validate_credentials(&existing.provider, cj)?;
        return dns_credential_repo::update(&db, &id, name.as_deref(), Some(cj));
    }

    let db = state.get_conn()?;
    dns_credential_repo::update(&db, &id, name.as_deref(), credentials_json.as_deref())
}

#[tauri::command]
pub async fn delete_dns_credential(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let db = state.get_conn()?;

    // Check if any ACME certificates reference this credential
    let referencing = cert_repo::find_by_dns_credential(&db, &id)?;
    if !referencing.is_empty() {
        return Err(AppError::Conflict(format!(
            "DNS credential is in use by {} certificate(s): {}",
            referencing.len(),
            referencing
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    dns_credential_repo::delete(&db, &id)
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}

#[tauri::command]
pub async fn test_dns_credential(
    id: String,
    state: State<'_, AppState>,
) -> Result<TestResult, AppError> {
    let cred = {
        let db = state.get_conn()?;
        dns_credential_repo::get_by_id(&db, &id)?
    };

    let provider = dns_provider::create_provider(&cred.provider, &cred.credentials_json)?;

    match provider.test_connection().await {
        Ok(msg) => Ok(TestResult {
            success: true,
            message: msg,
        }),
        Err(e) => Ok(TestResult {
            success: false,
            message: e.to_string(),
        }),
    }
}
