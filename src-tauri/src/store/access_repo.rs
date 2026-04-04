use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{
    AccessList, AccessListWithRules, AccessRule, CreateAccessList, CreateAccessRule,
};

fn row_to_access_list(row: &rusqlite::Row) -> rusqlite::Result<AccessList> {
    Ok(AccessList {
        id: row.get("id")?,
        name: row.get("name")?,
        default_policy: row.get("default_policy")?,
        created_at: row.get("created_at")?,
    })
}

fn row_to_access_rule(row: &rusqlite::Row) -> rusqlite::Result<AccessRule> {
    Ok(AccessRule {
        id: row.get("id")?,
        access_list_id: row.get("access_list_id")?,
        action: row.get("action")?,
        ip_cidr: row.get("ip_cidr")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
    })
}

// --- Access List CRUD ---

pub fn list_all_lists(conn: &Connection) -> Result<Vec<AccessList>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM access_lists ORDER BY name ASC")?;
    let lists = stmt
        .query_map([], |row| row_to_access_list(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lists)
}

pub fn get_list_by_id(conn: &Connection, id: &str) -> Result<AccessList, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM access_lists WHERE id = ?1")?;
    let list = stmt
        .query_row(params![id], |row| row_to_access_list(row))
        .map_err(|_| AppError::NotFound(format!("Access list '{}' not found", id)))?;
    Ok(list)
}

pub fn get_list_with_rules(conn: &Connection, id: &str) -> Result<AccessListWithRules, AppError> {
    let list = get_list_by_id(conn, id)?;
    let rules = list_rules_by_list(conn, id)?;
    Ok(AccessListWithRules { list, rules })
}

pub fn create_list(conn: &Connection, input: &CreateAccessList) -> Result<AccessList, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO access_lists (id, name, default_policy, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, input.name, input.default_policy, now],
    )?;

    get_list_by_id(conn, &id)
}

pub fn update_list(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    default_policy: Option<&str>,
) -> Result<AccessList, AppError> {
    let existing = get_list_by_id(conn, id)?;
    let name = name.unwrap_or(&existing.name);
    let policy = default_policy.unwrap_or(&existing.default_policy);

    conn.execute(
        "UPDATE access_lists SET name = ?1, default_policy = ?2 WHERE id = ?3",
        params![name, policy, id],
    )?;

    get_list_by_id(conn, id)
}

pub fn delete_list(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM access_lists WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Access list '{}' not found",
            id
        )));
    }
    Ok(())
}

// --- Access Rule CRUD ---

pub fn list_rules_by_list(conn: &Connection, list_id: &str) -> Result<Vec<AccessRule>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM access_rules WHERE access_list_id = ?1 ORDER BY sort_order ASC",
    )?;
    let rules = stmt
        .query_map(params![list_id], |row| row_to_access_rule(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

pub fn create_rule(conn: &Connection, input: &CreateAccessRule) -> Result<AccessRule, AppError> {
    // Validate the parent list exists
    get_list_by_id(conn, &input.access_list_id)?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let sort_order = input.sort_order.unwrap_or(0);

    conn.execute(
        "INSERT INTO access_rules (id, access_list_id, action, ip_cidr, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, input.access_list_id, input.action, input.ip_cidr, sort_order, now],
    )?;

    get_rule_by_id(conn, &id)
}

pub fn get_rule_by_id(conn: &Connection, id: &str) -> Result<AccessRule, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM access_rules WHERE id = ?1")?;
    let rule = stmt
        .query_row(params![id], |row| row_to_access_rule(row))
        .map_err(|_| AppError::NotFound(format!("Access rule '{}' not found", id)))?;
    Ok(rule)
}

pub fn delete_rule(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM access_rules WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Access rule '{}' not found",
            id
        )));
    }
    Ok(())
}

pub fn list_all_rules(conn: &Connection) -> Result<Vec<AccessRule>, AppError> {
    let mut stmt =
        conn.prepare("SELECT * FROM access_rules ORDER BY access_list_id, sort_order ASC")?;
    let rules = stmt
        .query_map([], |row| row_to_access_rule(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// Check if an access list with the given name already exists (case-insensitive).
pub fn find_by_name_ci(conn: &Connection, name: &str) -> Result<Option<AccessList>, AppError> {
    let mut stmt =
        conn.prepare("SELECT * FROM access_lists WHERE LOWER(name) = LOWER(?1)")?;
    let result = stmt
        .query_row(params![name], |row| row_to_access_list(row))
        .ok();
    Ok(result)
}

/// Check if a duplicate access rule exists (same list + action + ip_cidr).
pub fn find_duplicate_rule(
    conn: &Connection,
    access_list_id: &str,
    action: &str,
    ip_cidr: &str,
) -> Result<Option<AccessRule>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM access_rules WHERE access_list_id = ?1 AND action = ?2 AND ip_cidr = ?3",
    )?;
    let result = stmt
        .query_row(params![access_list_id, action, ip_cidr], |row| {
            row_to_access_rule(row)
        })
        .ok();
    Ok(result)
}

/// Reorder access rules by updating sort_order based on position in the given vec.
pub fn reorder_rules(
    conn: &Connection,
    access_list_id: &str,
    rule_ids: &[String],
) -> Result<(), AppError> {
    // Verify the access list exists
    get_list_by_id(conn, access_list_id)?;

    for (i, rule_id) in rule_ids.iter().enumerate() {
        let affected = conn.execute(
            "UPDATE access_rules SET sort_order = ?1 WHERE id = ?2 AND access_list_id = ?3",
            params![i as i32, rule_id, access_list_id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!(
                "Access rule '{}' not found in list '{}'",
                rule_id, access_list_id
            )));
        }
    }
    Ok(())
}
