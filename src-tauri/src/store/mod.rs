pub mod access_repo;
pub mod cert_repo;
pub mod models;
pub mod proxy_repo;
pub mod settings_repo;

use std::path::Path;

use rusqlite::Connection;
use tracing::info;

use crate::error::AppError;

/// Initialize database at the given path and run migrations.
pub fn init_database(db_path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    run_migrations(&conn)?;
    info!("Database initialized at {:?}", db_path);
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS proxy_rules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            proxy_type TEXT NOT NULL CHECK(proxy_type IN ('http','stream_tcp','stream_udp')),
            enabled INTEGER NOT NULL DEFAULT 1,
            listen_port INTEGER NOT NULL,
            listen_host TEXT DEFAULT '0.0.0.0',
            domain TEXT,
            path_prefix TEXT,
            upstream_host TEXT NOT NULL,
            upstream_port INTEGER NOT NULL,
            tls_mode TEXT DEFAULT 'none' CHECK(tls_mode IN ('none','terminate','passthrough')),
            certificate_id TEXT REFERENCES certificates(id),
            access_list_id TEXT REFERENCES access_lists(id),
            websocket INTEGER DEFAULT 0,
            custom_headers TEXT,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS certificates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            domain TEXT NOT NULL,
            cert_path TEXT NOT NULL,
            key_path TEXT NOT NULL,
            source TEXT NOT NULL CHECK(source IN ('upload','self_signed','acme')),
            expires_at TEXT NOT NULL,
            auto_renew INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS access_lists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            default_policy TEXT NOT NULL CHECK(default_policy IN ('allow','deny')),
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS access_rules (
            id TEXT PRIMARY KEY,
            access_list_id TEXT NOT NULL REFERENCES access_lists(id) ON DELETE CASCADE,
            action TEXT NOT NULL CHECK(action IN ('allow','deny')),
            ip_cidr TEXT NOT NULL,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    info!("Database migrations complete");
    Ok(())
}

/// Create a backup of the database file.
pub fn backup_database(db_path: &Path) -> Result<String, AppError> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!(
        "{}.backup_{}.db",
        db_path.file_stem().unwrap_or_default().to_string_lossy(),
        timestamp
    );
    let backup_path = db_path.with_file_name(&backup_name);
    std::fs::copy(db_path, &backup_path)?;
    info!("Database backed up to {:?}", backup_path);
    Ok(backup_path.to_string_lossy().to_string())
}
