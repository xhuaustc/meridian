// SPEC: FEAT-001-acme-dns/spec.md | T-001, T-006
use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{CreateDnsCredential, DnsCredential};

fn row_to_cred(row: &rusqlite::Row) -> rusqlite::Result<DnsCredential> {
    Ok(DnsCredential {
        id: row.get("id")?,
        name: row.get("name")?,
        provider: row.get("provider")?,
        credentials_json: row.get("credentials_json")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_all(conn: &Connection) -> Result<Vec<DnsCredential>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM dns_credentials ORDER BY created_at DESC")?;
    let creds = stmt
        .query_map([], |row| row_to_cred(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(creds)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<DnsCredential, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM dns_credentials WHERE id = ?1")?;
    let cred = stmt
        .query_row(params![id], |row| row_to_cred(row))
        .map_err(|_| AppError::NotFound(format!("DNS credential '{}' not found", id)))?;
    Ok(cred)
}

pub fn create(conn: &Connection, input: &CreateDnsCredential) -> Result<DnsCredential, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO dns_credentials (id, name, provider, credentials_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            input.name,
            input.provider,
            input.credentials_json,
            now,
            now
        ],
    )?;

    get_by_id(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    credentials_json: Option<&str>,
) -> Result<DnsCredential, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    // Verify exists
    get_by_id(conn, id)?;

    if let Some(n) = name {
        conn.execute(
            "UPDATE dns_credentials SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![n, now, id],
        )?;
    }
    if let Some(cj) = credentials_json {
        conn.execute(
            "UPDATE dns_credentials SET credentials_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![cj, now, id],
        )?;
    }

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM dns_credentials WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "DNS credential '{}' not found",
            id
        )));
    }
    Ok(())
}
