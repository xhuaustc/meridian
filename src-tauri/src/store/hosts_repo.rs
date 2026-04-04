use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{CreateHostEntry, HostEntry};

fn row_to_host_entry(row: &rusqlite::Row) -> rusqlite::Result<HostEntry> {
    Ok(HostEntry {
        id: row.get("id")?,
        ip: row.get("ip")?,
        hostname: row.get("hostname")?,
        comment: row.get("comment")?,
        enabled: row.get("enabled")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_all(conn: &Connection, keyword: Option<&str>) -> Result<Vec<HostEntry>, AppError> {
    if let Some(kw) = keyword {
        let pattern = format!("%{}%", kw);
        let mut stmt = conn.prepare(
            "SELECT * FROM host_entries WHERE hostname LIKE ?1 OR ip LIKE ?1 OR comment LIKE ?1 ORDER BY hostname ASC",
        )?;
        let entries = stmt
            .query_map(params![pattern], |row| row_to_host_entry(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    } else {
        let mut stmt = conn.prepare("SELECT * FROM host_entries ORDER BY hostname ASC")?;
        let entries = stmt
            .query_map([], |row| row_to_host_entry(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<HostEntry, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM host_entries WHERE id = ?1")?;
    let entry = stmt
        .query_row(params![id], |row| row_to_host_entry(row))
        .map_err(|_| AppError::NotFound(format!("Host entry '{}' not found", id)))?;
    Ok(entry)
}

pub fn find_by_hostname(conn: &Connection, hostname: &str) -> Result<Option<HostEntry>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM host_entries WHERE LOWER(hostname) = LOWER(?1)")?;
    let result = stmt
        .query_row(params![hostname], |row| row_to_host_entry(row))
        .ok();
    Ok(result)
}

pub fn create(conn: &Connection, input: &CreateHostEntry) -> Result<HostEntry, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO host_entries (id, ip, hostname, comment, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
        params![id, input.ip, input.hostname, input.comment, now],
    )?;

    get_by_id(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    ip: Option<&str>,
    hostname: Option<&str>,
    comment: Option<&str>,
) -> Result<HostEntry, AppError> {
    let existing = get_by_id(conn, id)?;
    let ip = ip.unwrap_or(&existing.ip);
    let hostname = hostname.unwrap_or(&existing.hostname);
    let comment = comment.or(existing.comment.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE host_entries SET ip = ?1, hostname = ?2, comment = ?3, updated_at = ?4 WHERE id = ?5",
        params![ip, hostname, comment, now, id],
    )?;

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM host_entries WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Host entry '{}' not found",
            id
        )));
    }
    Ok(())
}

pub fn toggle(conn: &Connection, id: &str, enabled: bool) -> Result<HostEntry, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE host_entries SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        params![enabled, now, id],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Host entry '{}' not found",
            id
        )));
    }
    get_by_id(conn, id)
}

pub fn list_enabled(conn: &Connection) -> Result<Vec<HostEntry>, AppError> {
    let mut stmt =
        conn.prepare("SELECT * FROM host_entries WHERE enabled = 1 ORDER BY hostname ASC")?;
    let entries = stmt
        .query_map([], |row| row_to_host_entry(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}
