// SPEC: FEAT-001-acme-dns/spec.md | T-001
use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::AcmeAccount;

fn row_to_account(row: &rusqlite::Row) -> rusqlite::Result<AcmeAccount> {
    Ok(AcmeAccount {
        id: row.get("id")?,
        email: row.get("email")?,
        account_key_pem: row.get("account_key_pem")?,
        ca_url: row.get("ca_url")?,
        created_at: row.get("created_at")?,
    })
}

pub fn find_by_email(conn: &Connection, email: &str) -> Result<Option<AcmeAccount>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM acme_accounts WHERE email = ?1")?;
    let result = stmt.query_row(params![email], |row| row_to_account(row));
    match result {
        Ok(acc) => Ok(Some(acc)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

pub fn create(
    conn: &Connection,
    email: &str,
    account_key_pem: &str,
    ca_url: &str,
) -> Result<AcmeAccount, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO acme_accounts (id, email, account_key_pem, ca_url, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, email, account_key_pem, ca_url, now],
    )?;

    let mut stmt = conn.prepare("SELECT * FROM acme_accounts WHERE id = ?1")?;
    let acc = stmt
        .query_row(params![id], |row| row_to_account(row))
        .map_err(|_| AppError::NotFound("ACME account not found after create".to_string()))?;
    Ok(acc)
}
