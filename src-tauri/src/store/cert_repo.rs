use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{Certificate, CreateCertificate};

fn row_to_cert(row: &rusqlite::Row) -> rusqlite::Result<Certificate> {
    Ok(Certificate {
        id: row.get("id")?,
        name: row.get("name")?,
        domain: row.get("domain")?,
        cert_path: row.get("cert_path")?,
        key_path: row.get("key_path")?,
        source: row.get("source")?,
        expires_at: row.get("expires_at")?,
        auto_renew: row.get::<_, i32>("auto_renew")? != 0,
        created_at: row.get("created_at")?,
        dns_credential_id: row.get("dns_credential_id").unwrap_or(None),
        acme_account_id: row.get("acme_account_id").unwrap_or(None),
        acme_domains: row.get("acme_domains").unwrap_or(None),
        last_renew_error: row.get("last_renew_error").unwrap_or(None),
        last_renew_at: row.get("last_renew_at").unwrap_or(None),
        status: row.get("status").unwrap_or_else(|_| "ready".to_string()),
    })
}

pub fn list_all(conn: &Connection) -> Result<Vec<Certificate>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM certificates ORDER BY created_at DESC")?;
    let certs = stmt.query_map([], |row| row_to_cert(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Certificate, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM certificates WHERE id = ?1")?;
    let cert = stmt.query_row(params![id], |row| row_to_cert(row))
        .map_err(|_| AppError::NotFound(format!("Certificate '{}' not found", id)))?;
    Ok(cert)
}

pub fn create(conn: &Connection, input: &CreateCertificate) -> Result<Certificate, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let auto_renew = if input.auto_renew.unwrap_or(false) { 1 } else { 0 };

    conn.execute(
        "INSERT INTO certificates (id, name, domain, cert_path, key_path, source, expires_at, auto_renew, created_at, dns_credential_id, acme_account_id, acme_domains)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            input.name,
            input.domain,
            input.cert_path,
            input.key_path,
            input.source,
            input.expires_at,
            auto_renew,
            now,
            input.dns_credential_id,
            input.acme_account_id,
            input.acme_domains,
        ],
    )?;

    get_by_id(conn, &id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM certificates WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Certificate '{}' not found", id)));
    }
    Ok(())
}

pub fn update_cert_after_renewal(
    conn: &Connection,
    id: &str,
    expires_at: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(err_msg) = error {
        conn.execute(
            "UPDATE certificates SET last_renew_at = ?1, last_renew_error = ?2 WHERE id = ?3",
            params![now, err_msg, id],
        )?;
    } else {
        conn.execute(
            "UPDATE certificates SET expires_at = ?1, last_renew_at = ?2, last_renew_error = NULL WHERE id = ?3",
            params![expires_at, now, id],
        )?;
    }
    Ok(())
}

pub fn list_acme_auto_renew(conn: &Connection) -> Result<Vec<Certificate>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM certificates WHERE source = 'acme' AND auto_renew = 1 ORDER BY expires_at ASC",
    )?;
    let certs = stmt.query_map([], |row| row_to_cert(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

pub fn find_by_dns_credential(conn: &Connection, dns_credential_id: &str) -> Result<Vec<Certificate>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM certificates WHERE dns_credential_id = ?1",
    )?;
    let certs = stmt.query_map(params![dns_credential_id], |row| row_to_cert(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

/// Create a pending certificate placeholder (no cert/key data yet).
pub fn create_pending(
    conn: &Connection,
    name: &str,
    domain: &str,
    auto_renew: bool,
    dns_credential_id: &str,
    acme_domains: &str,
) -> Result<Certificate, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let ar = if auto_renew { 1 } else { 0 };

    conn.execute(
        "INSERT INTO certificates (id, name, domain, cert_path, key_path, source, expires_at, auto_renew, created_at, dns_credential_id, acme_account_id, acme_domains, status)
         VALUES (?1, ?2, ?3, '', '', 'acme', ?4, ?5, ?4, ?6, NULL, ?7, 'pending')",
        params![id, name, domain, now, ar, dns_credential_id, acme_domains],
    )?;

    get_by_id(conn, &id)
}

/// Update a pending cert to ready with actual cert data.
pub fn finish_pending(
    conn: &Connection,
    id: &str,
    cert_path: &str,
    key_path: &str,
    expires_at: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE certificates SET cert_path = ?1, key_path = ?2, expires_at = ?3, status = 'ready' WHERE id = ?4",
        params![cert_path, key_path, expires_at, id],
    )?;
    Ok(())
}

/// Mark a pending cert as failed.
pub fn fail_pending(conn: &Connection, id: &str, error: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE certificates SET status = 'failed', last_renew_error = ?1 WHERE id = ?2",
        params![error, id],
    )?;
    Ok(())
}

pub fn get_expiring(conn: &Connection, within_days: i64) -> Result<Vec<Certificate>, AppError> {
    let threshold = (chrono::Utc::now() + chrono::Duration::days(within_days)).to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT * FROM certificates WHERE expires_at <= ?1 ORDER BY expires_at ASC",
    )?;
    let certs = stmt.query_map(params![threshold], |row| row_to_cert(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
