use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppError;
use crate::store::models::HostnameSettings;

fn row_to_hostname_settings(row: &rusqlite::Row) -> rusqlite::Result<HostnameSettings> {
    Ok(HostnameSettings {
        hostname: row.get("hostname")?,
        hsts_enabled: row.get("hsts_enabled")?,
        hsts_max_age: row.get("hsts_max_age")?,
        hsts_include_subdomains: row.get("hsts_include_subdomains")?,
        sec_nosniff: row.get("sec_nosniff")?,
        sec_referrer_policy: row.get("sec_referrer_policy")?,
        sec_frame_options: row.get("sec_frame_options")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Load every hostname override, keyed by hostname.
///
/// The generator looks rows up by the hostname of the server block it is
/// writing; a hostname that is absent here keeps the defaults.
pub fn list_all(conn: &Connection) -> Result<HashMap<String, HostnameSettings>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM hostname_settings ORDER BY hostname ASC")?;
    let settings = stmt
        .query_map([], |row| row_to_hostname_settings(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(settings
        .into_iter()
        .map(|settings| (settings.hostname.clone(), settings))
        .collect())
}

/// Load one hostname's override row.
pub fn get(conn: &Connection, hostname: &str) -> Result<Option<HostnameSettings>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM hostname_settings WHERE hostname = ?1")?;
    let settings = stmt
        .query_row(params![hostname], row_to_hostname_settings)
        .optional()?;
    Ok(settings)
}

/// Store the HSTS switch for a hostname, creating the row on first use.
///
/// This is the only mutator the app has for `hostname_settings`, and it touches
/// only the HSTS columns (plus `updated_at`). The three security-header
/// switches keep whatever value they already have, and a newly created row
/// takes the column defaults (all three on). HSTS itself defaults off, with a
/// 300 second `max-age`, which is why a first write is safe to undo.
pub fn set_hsts(
    conn: &Connection,
    hostname: &str,
    enabled: bool,
    max_age: i64,
    include_subdomains: bool,
) -> Result<HostnameSettings, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO hostname_settings
             (hostname, hsts_enabled, hsts_max_age, hsts_include_subdomains,
              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(hostname) DO UPDATE SET
             hsts_enabled = excluded.hsts_enabled,
             hsts_max_age = excluded.hsts_max_age,
             hsts_include_subdomains = excluded.hsts_include_subdomains,
             updated_at = excluded.updated_at",
        params![hostname, enabled, max_age, include_subdomains, now],
    )?;
    get(conn, hostname)?
        .ok_or_else(|| AppError::NotFound(format!("Hostname settings for '{}'", hostname)))
}

/// Delete one settings row only if no proxy rule names its hostname.
///
/// The reference check lives in the DELETE statement itself, not in a caller
/// side map: cleanup must be unable to remove a row that a rule still uses,
/// even if the row list it started from was stale.
fn delete_if_unreferenced(conn: &Connection, hostname: &str) -> Result<bool, AppError> {
    let deleted = conn.execute(
        "DELETE FROM hostname_settings
         WHERE hostname = ?1
           AND NOT EXISTS (
               SELECT 1 FROM proxy_rules WHERE proxy_rules.domain = ?1
           )",
        params![hostname],
    )?;
    Ok(deleted > 0)
}

/// Delete every settings row that no proxy rule references any more.
///
/// This is the settings page's cleanup action. Returns the hostnames that were
/// deleted, in list order, so the caller can report how many rows went away.
/// A row referenced by a rule — even a disabled rule — is never touched.
pub fn delete_orphans(conn: &Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT hostname FROM hostname_settings ORDER BY hostname ASC")?;
    let hostnames = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut deleted = Vec::new();
    for hostname in hostnames {
        if delete_if_unreferenced(conn, &hostname)? {
            deleted.push(hostname);
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_all_keys_rows_by_hostname_and_reads_every_switch() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO hostname_settings
                 (hostname, hsts_enabled, hsts_max_age, hsts_include_subdomains,
                  sec_nosniff, sec_referrer_policy, sec_frame_options, created_at, updated_at)
             VALUES
                 ('plain.local', 0, 300, 0, 1, 1, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
                 ('tight.local', 1, 31536000, 1, 0, 1, 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        )
        .unwrap();

        let settings = list_all(&conn).unwrap();
        assert_eq!(settings.len(), 2);

        let plain = settings.get("plain.local").expect("plain row");
        assert!(plain.sec_nosniff);
        assert!(plain.sec_referrer_policy);
        assert!(plain.sec_frame_options);
        assert!(!plain.hsts_enabled);
        assert_eq!(plain.hsts_max_age, 300);

        let tight = settings.get("tight.local").expect("tight row");
        assert!(!tight.sec_nosniff);
        assert!(tight.sec_referrer_policy);
        assert!(!tight.sec_frame_options);
        assert!(tight.hsts_enabled);
        assert_eq!(tight.hsts_max_age, 31_536_000);
        assert!(tight.hsts_include_subdomains);
    }

    #[test]
    fn get_returns_none_until_a_row_exists() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        assert!(get(&conn, "app.local").unwrap().is_none());
    }

    #[test]
    fn set_hsts_creates_the_row_with_safe_defaults_for_the_other_columns() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();

        let stored = set_hsts(&conn, "app.local", true, 600, true).unwrap();
        assert!(stored.hsts_enabled);
        assert_eq!(stored.hsts_max_age, 600);
        assert!(stored.hsts_include_subdomains);
        assert!(
            stored.sec_nosniff && stored.sec_referrer_policy && stored.sec_frame_options,
            "creating an HSTS row must not turn the default headers off"
        );

        let reread = get(&conn, "app.local").unwrap().expect("row");
        assert_eq!(reread.hsts_max_age, 600);
        assert!(reread.hsts_include_subdomains);
    }

    #[test]
    fn set_hsts_updates_only_the_hsts_columns_and_can_turn_it_off() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO hostname_settings
                 (hostname, hsts_enabled, hsts_max_age, hsts_include_subdomains,
                  sec_nosniff, sec_referrer_policy, sec_frame_options, created_at, updated_at)
             VALUES ('app.local', 1, 300, 0, 0, 1, 0,
                     '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let stored = set_hsts(&conn, "app.local", false, 60, false).unwrap();
        assert!(!stored.hsts_enabled, "HSTS must be turnable off again");
        assert_eq!(stored.hsts_max_age, 60);
        assert!(!stored.hsts_include_subdomains);
        assert!(
            !stored.sec_nosniff && stored.sec_referrer_policy && !stored.sec_frame_options,
            "the security-header switches are not this function's business"
        );
        assert_eq!(
            stored.created_at, "2024-01-01T00:00:00Z",
            "updating HSTS must not rewrite the creation time"
        );
        assert_ne!(stored.updated_at, "2024-01-01T00:00:00Z");
    }

    fn insert_rule(conn: &Connection, id: &str, domain: &str, enabled: bool) {
        conn.execute(
            "INSERT INTO proxy_rules
                 (id, name, proxy_type, enabled, listen_port, domain,
                  upstream_host, upstream_port, created_at, updated_at)
             VALUES (?1, ?1, 'http', ?2, 18080, ?3,
                     '127.0.0.1', 18081, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            rusqlite::params![id, if enabled { 1 } else { 0 }, domain],
        )
        .unwrap();
    }

    fn insert_settings(conn: &Connection, hostname: &str, hsts_enabled: bool) {
        conn.execute(
            "INSERT INTO hostname_settings (hostname, hsts_enabled, created_at, updated_at)
             VALUES (?1, ?2, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            rusqlite::params![hostname, if hsts_enabled { 1 } else { 0 }],
        )
        .unwrap();
    }

    #[test]
    fn delete_orphans_removes_only_rows_no_rule_names() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        insert_rule(&conn, "rule-app", "app.local", true);
        insert_rule(&conn, "rule-disabled", "disabled.local", false);
        insert_settings(&conn, "app.local", true);
        insert_settings(&conn, "disabled.local", false);
        insert_settings(&conn, "orphan.local", true);

        let deleted = delete_orphans(&conn).unwrap();
        assert_eq!(deleted, vec!["orphan.local".to_string()]);

        let remaining = list_all(&conn).unwrap();
        assert!(remaining.contains_key("app.local"));
        assert!(
            remaining.contains_key("disabled.local"),
            "a disabled rule still references its hostname, so cleanup keeps the row"
        );
        assert!(!remaining.contains_key("orphan.local"));
    }

    #[test]
    fn cleanup_refuses_to_delete_a_row_a_rule_still_references() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        insert_rule(&conn, "rule-app", "app.local", true);
        insert_settings(&conn, "app.local", true);

        assert!(!delete_if_unreferenced(&conn, "app.local").unwrap());
        assert!(
            get(&conn, "app.local").unwrap().is_some(),
            "a referenced row must survive even when cleanup is handed its name"
        );
    }

    #[test]
    fn delete_orphans_on_an_empty_database_returns_nothing() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        assert!(delete_orphans(&conn).unwrap().is_empty());
        assert!(list_all(&conn).unwrap().is_empty());
    }

    #[test]
    fn overview_through_the_repositories_is_empty_on_a_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();

        let settings = list_all(&conn).unwrap();
        let rules = crate::store::proxy_repo::list_all(&conn).unwrap();
        let overview = crate::config_engine::hostname_settings::overview(&settings, &rules);

        assert!(overview.entries.is_empty());
        assert_eq!(overview.orphan_count, 0);
        assert_eq!(overview.orphan_hsts_count, 0);
    }
}
