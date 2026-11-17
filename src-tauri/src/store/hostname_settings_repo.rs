use std::collections::HashMap;

use rusqlite::Connection;

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
}
