use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{CreateProxyRule, ProxyRule, UpdateProxyRule};

fn row_to_proxy(row: &rusqlite::Row) -> rusqlite::Result<ProxyRule> {
    Ok(ProxyRule {
        id: row.get("id")?,
        name: row.get("name")?,
        proxy_type: row.get("proxy_type")?,
        enabled: row.get::<_, i32>("enabled")? != 0,
        listen_port: row.get::<_, u32>("listen_port")? as u16,
        listen_host: row.get::<_, String>("listen_host")?,
        domain: row.get("domain")?,
        path_prefix: row.get("path_prefix")?,
        upstream_host: row.get("upstream_host")?,
        upstream_port: row.get::<_, u32>("upstream_port")? as u16,
        upstream_scheme: row.get::<_, String>("upstream_scheme")?,
        tls_mode: row.get::<_, String>("tls_mode")?,
        certificate_id: row.get("certificate_id")?,
        access_list_id: row.get("access_list_id")?,
        websocket: row.get::<_, i32>("websocket")? != 0,
        keep_alive: row.get::<_, i32>("keep_alive")? != 0,
        custom_headers: row.get("custom_headers")?,
        upstream_targets: row.get("upstream_targets")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_all(conn: &Connection) -> Result<Vec<ProxyRule>, AppError> {
    let mut stmt =
        conn.prepare("SELECT * FROM proxy_rules ORDER BY sort_order ASC, created_at ASC")?;
    let rules = stmt
        .query_map([], |row| row_to_proxy(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<ProxyRule, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM proxy_rules WHERE id = ?1")?;
    let rule = stmt
        .query_row(params![id], |row| row_to_proxy(row))
        .map_err(|_| AppError::NotFound(format!("Proxy rule '{}' not found", id)))?;
    Ok(rule)
}

pub fn create(conn: &Connection, input: &CreateProxyRule) -> Result<ProxyRule, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let listen_host = input
        .listen_host
        .clone()
        .unwrap_or_else(|| "0.0.0.0".to_string());
    let upstream_scheme = input
        .upstream_scheme
        .clone()
        .unwrap_or_else(|| "http".to_string());
    let tls_mode = input.tls_mode.clone().unwrap_or_else(|| "none".to_string());
    let websocket = if input.websocket.unwrap_or(false) {
        1
    } else {
        0
    };
    let keep_alive = if input.keep_alive.unwrap_or(false) {
        1
    } else {
        0
    };
    let sort_order = input.sort_order.unwrap_or(0);

    conn.execute(
        "INSERT INTO proxy_rules (id, name, proxy_type, enabled, listen_port, listen_host, domain, path_prefix, upstream_host, upstream_port, upstream_scheme, tls_mode, certificate_id, access_list_id, websocket, keep_alive, custom_headers, upstream_targets, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            id,
            input.name,
            input.proxy_type,
            input.listen_port as u32,
            listen_host,
            input.domain,
            input.path_prefix,
            input.upstream_host,
            input.upstream_port as u32,
            upstream_scheme,
            tls_mode,
            input.certificate_id,
            input.access_list_id,
            websocket,
            keep_alive,
            input.custom_headers,
            input.upstream_targets,
            sort_order,
            now,
            now,
        ],
    )?;

    get_by_id(conn, &id)
}

pub fn update(conn: &Connection, id: &str, input: &UpdateProxyRule) -> Result<ProxyRule, AppError> {
    // Ensure the rule exists first
    let existing = get_by_id(conn, id)?;
    let now = chrono::Utc::now().to_rfc3339();

    // For non-optional fields, fall back to existing value if not provided.
    // For optional fields (domain, path_prefix, certificate_id, access_list_id,
    // custom_headers), use the input value directly so the frontend can explicitly
    // clear them by sending None.
    let name = input.name.as_deref().unwrap_or(&existing.name);
    let proxy_type = input.proxy_type.as_deref().unwrap_or(&existing.proxy_type);
    let enabled = input.enabled.unwrap_or(existing.enabled);
    let listen_port = input.listen_port.unwrap_or(existing.listen_port);
    let listen_host = input
        .listen_host
        .as_deref()
        .unwrap_or(&existing.listen_host);
    let upstream_host = input
        .upstream_host
        .as_deref()
        .unwrap_or(&existing.upstream_host);
    let upstream_port = input.upstream_port.unwrap_or(existing.upstream_port);
    let upstream_scheme = input
        .upstream_scheme
        .as_deref()
        .unwrap_or(&existing.upstream_scheme);
    let tls_mode = input.tls_mode.as_deref().unwrap_or(&existing.tls_mode);
    let websocket = input.websocket.unwrap_or(existing.websocket);
    let keep_alive = input.keep_alive.unwrap_or(existing.keep_alive);
    let sort_order = input.sort_order.unwrap_or(existing.sort_order);

    // Optional fields: always use value from input (allows clearing by sending null)
    let domain: &Option<String> = &input.domain;
    let path_prefix: &Option<String> = &input.path_prefix;
    let certificate_id: &Option<String> = &input.certificate_id;
    let access_list_id: &Option<String> = &input.access_list_id;
    let custom_headers: &Option<String> = &input.custom_headers;
    let upstream_targets: &Option<String> = &input.upstream_targets;

    conn.execute(
        "UPDATE proxy_rules SET name=?1, proxy_type=?2, enabled=?3, listen_port=?4, listen_host=?5, domain=?6, path_prefix=?7, upstream_host=?8, upstream_port=?9, upstream_scheme=?10, tls_mode=?11, certificate_id=?12, access_list_id=?13, websocket=?14, keep_alive=?15, custom_headers=?16, upstream_targets=?17, sort_order=?18, updated_at=?19 WHERE id=?20",
        params![
            name,
            proxy_type,
            if enabled { 1 } else { 0 },
            listen_port as u32,
            listen_host,
            domain,
            path_prefix,
            upstream_host,
            upstream_port as u32,
            upstream_scheme,
            tls_mode,
            certificate_id,
            access_list_id,
            if websocket { 1 } else { 0 },
            if keep_alive { 1 } else { 0 },
            custom_headers,
            upstream_targets,
            sort_order,
            now,
            id,
        ],
    )?;

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM proxy_rules WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Proxy rule '{}' not found", id)));
    }
    Ok(())
}

pub fn toggle_enabled(conn: &Connection, id: &str, enabled: bool) -> Result<ProxyRule, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE proxy_rules SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        params![if enabled { 1 } else { 0 }, now, id],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Proxy rule '{}' not found", id)));
    }
    get_by_id(conn, id)
}

pub fn list_enabled(conn: &Connection) -> Result<Vec<ProxyRule>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM proxy_rules WHERE enabled = 1 ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rules = stmt
        .query_map([], |row| row_to_proxy(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// Count proxy rules grouped by proxy_type.
pub fn count_by_type(conn: &Connection) -> Result<HashMap<String, i64>, AppError> {
    let mut stmt =
        conn.prepare("SELECT proxy_type, COUNT(*) as cnt FROM proxy_rules GROUP BY proxy_type")?;
    let mut map = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (ptype, count) = row?;
        map.insert(ptype, count);
    }
    Ok(map)
}

/// Batch toggle enabled status for multiple proxy rules.
pub fn batch_toggle(conn: &Connection, ids: &[String], enabled: bool) -> Result<usize, AppError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().to_rfc3339();
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "UPDATE proxy_rules SET enabled = ?{}, updated_at = ?{} WHERE id IN ({})",
        ids.len() + 1,
        ids.len() + 2,
        placeholders.join(",")
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in ids {
        param_values.push(Box::new(id.clone()));
    }
    param_values.push(Box::new(if enabled { 1i32 } else { 0i32 }));
    param_values.push(Box::new(now));
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_refs.as_slice())?;
    Ok(affected)
}

/// Batch delete multiple proxy rules.
pub fn batch_delete(conn: &Connection, ids: &[String]) -> Result<usize, AppError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "DELETE FROM proxy_rules WHERE id IN ({})",
        placeholders.join(",")
    );
    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = ids
        .iter()
        .map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, params_refs.as_slice())?;
    Ok(affected)
}

/// Find proxy rules that reference a given certificate_id.
pub fn find_by_certificate(
    conn: &Connection,
    certificate_id: &str,
) -> Result<Vec<ProxyRule>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM proxy_rules WHERE certificate_id = ?1")?;
    let rules = stmt
        .query_map(params![certificate_id], |row| row_to_proxy(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// Find proxy rules that reference a given access_list_id.
pub fn find_by_access_list(
    conn: &Connection,
    access_list_id: &str,
) -> Result<Vec<ProxyRule>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM proxy_rules WHERE access_list_id = ?1")?;
    let rules = stmt
        .query_map(params![access_list_id], |row| row_to_proxy(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// List proxy rules with optional filters.
pub fn list_filtered(
    conn: &Connection,
    proxy_type: Option<&str>,
    enabled: Option<bool>,
    search: Option<&str>,
) -> Result<Vec<ProxyRule>, AppError> {
    let mut sql = "SELECT * FROM proxy_rules WHERE 1=1".to_string();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(pt) = proxy_type {
        param_values.push(Box::new(pt.to_string()));
        sql.push_str(&format!(" AND proxy_type = ?{}", param_values.len()));
    }
    if let Some(en) = enabled {
        param_values.push(Box::new(if en { 1i32 } else { 0i32 }));
        sql.push_str(&format!(" AND enabled = ?{}", param_values.len()));
    }
    if let Some(s) = search {
        let pattern = format!("%{}%", s);
        param_values.push(Box::new(pattern));
        sql.push_str(&format!(
            " AND (name LIKE ?{0} OR domain LIKE ?{0})",
            param_values.len()
        ));
    }
    sql.push_str(" ORDER BY sort_order ASC, created_at ASC");

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let rules = stmt
        .query_map(params_refs.as_slice(), |row| row_to_proxy(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}
