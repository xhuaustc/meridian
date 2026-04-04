use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::AppSetting;

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
    let result = stmt.query_row(params![key], |row| row.get::<_, String>(0));
    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn delete(conn: &Connection, key: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM app_settings WHERE key = ?1", params![key])?;
    Ok(())
}

pub fn list_all(conn: &Connection) -> Result<Vec<AppSetting>, AppError> {
    let mut stmt = conn.prepare("SELECT key, value FROM app_settings ORDER BY key ASC")?;
    let settings = stmt
        .query_map([], |row| {
            Ok(AppSetting {
                key: row.get("key")?,
                value: row.get("value")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(settings)
}
