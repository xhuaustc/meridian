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
        "INSERT INTO certificates (id, name, domain, cert_path, key_path, source, expires_at, auto_renew, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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

pub fn get_expiring(conn: &Connection, within_days: i64) -> Result<Vec<Certificate>, AppError> {
    let threshold = (chrono::Utc::now() + chrono::Duration::days(within_days)).to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT * FROM certificates WHERE expires_at <= ?1 ORDER BY expires_at ASC",
    )?;
    let certs = stmt.query_map(params![threshold], |row| row_to_cert(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
