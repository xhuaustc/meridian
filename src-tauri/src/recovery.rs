use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use base64::Engine;
use ring::rand::SecureRandom;
use ring::{aead, pbkdf2, rand};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config_engine;
use crate::error::AppError;
use crate::store;

const MAGIC: &[u8; 8] = b"MRDNREC1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const MAX_BUNDLE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct RecoveryPreview {
    pub exported_at: String,
    pub proxy_count: i64,
    pub certificate_count: i64,
    pub host_count: i64,
    pub dns_credential_count: i64,
}

#[derive(Serialize, Deserialize)]
struct CertMaterial {
    id: String,
    cert: String,
    key: String,
}

#[derive(Serialize, Deserialize)]
struct RecoveryBundle {
    version: u32,
    preview: RecoveryPreview,
    database: String,
    certificates: Vec<CertMaterial>,
}

fn recovery_error(message: &str) -> AppError {
    AppError::Validation(message.to_string())
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<aead::LessSafeKey, AppError> {
    if passphrase.chars().count() < 12 {
        return Err(recovery_error(
            "Recovery passphrase must be at least 12 characters",
        ));
    }
    let mut bytes = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(200_000).expect("constant is nonzero"),
        salt,
        passphrase.as_bytes(),
        &mut bytes,
    );
    let key = aead::UnboundKey::new(&aead::AES_256_GCM, &bytes)
        .map_err(|_| recovery_error("Could not initialize recovery encryption"))?;
    Ok(aead::LessSafeKey::new(key))
}

fn encrypt_bundle(bundle: &RecoveryBundle, passphrase: &str) -> Result<Vec<u8>, AppError> {
    let random = rand::SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    random
        .fill(&mut salt)
        .map_err(|_| recovery_error("Could not generate recovery salt"))?;
    random
        .fill(&mut nonce)
        .map_err(|_| recovery_error("Could not generate recovery nonce"))?;
    let key = derive_key(passphrase, &salt)?;
    let mut payload = serde_json::to_vec(bundle)?;
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(&MAGIC[..]),
        &mut payload,
    )
    .map_err(|_| recovery_error("Could not encrypt recovery bundle"))?;
    let mut output = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + payload.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&payload);
    Ok(output)
}

fn read_bundle(path: &Path, passphrase: &str) -> Result<RecoveryBundle, AppError> {
    let mut input = Vec::new();
    File::open(path)?
        .take(MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut input)?;
    if input.len() as u64 > MAX_BUNDLE_BYTES
        || input.len() < MAGIC.len() + SALT_LEN + NONCE_LEN + aead::AES_256_GCM.tag_len()
        || &input[..MAGIC.len()] != MAGIC
    {
        return Err(recovery_error("Invalid or oversized recovery bundle"));
    }
    let salt_start = MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let cipher_start = nonce_start + NONCE_LEN;
    let nonce: [u8; NONCE_LEN] = input[nonce_start..cipher_start]
        .try_into()
        .map_err(|_| recovery_error("Invalid recovery nonce"))?;
    let key = derive_key(passphrase, &input[salt_start..nonce_start])?;
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(&MAGIC[..]),
            &mut input[cipher_start..],
        )
        .map_err(|_| recovery_error("Wrong passphrase or damaged recovery bundle"))?;
    let bundle: RecoveryBundle = serde_json::from_slice(plaintext)?;
    if bundle.version != 1 {
        return Err(recovery_error("Unsupported recovery bundle version"));
    }
    Ok(bundle)
}

fn count(conn: &Connection, table: &str) -> Result<i64, AppError> {
    let sql = format!("SELECT COUNT(*) FROM {}", table);
    Ok(conn.query_row(&sql, [], |row| row.get(0))?)
}

fn secure_new_file(path: &Path) -> Result<File, AppError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

pub fn create_bundle(
    db_path: &Path,
    save_path: &Path,
    passphrase: &str,
) -> Result<RecoveryPreview, AppError> {
    if passphrase.chars().count() < 12 {
        return Err(recovery_error(
            "Recovery passphrase must be at least 12 characters",
        ));
    }
    let snapshot_path = PathBuf::from(store::backup_database(db_path)?);
    let result = (|| -> Result<RecoveryPreview, AppError> {
        let snapshot = Connection::open(&snapshot_path)?;
        let preview = RecoveryPreview {
            exported_at: chrono::Utc::now().to_rfc3339(),
            proxy_count: count(&snapshot, "proxy_rules")?,
            certificate_count: count(&snapshot, "certificates")?,
            host_count: count(&snapshot, "host_entries")?,
            dns_credential_count: count(&snapshot, "dns_credentials")?,
        };
        let mut stmt = snapshot
            .prepare("SELECT id, cert_path, key_path FROM certificates WHERE status = 'ready'")?;
        let paths = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut certificates = Vec::with_capacity(paths.len());
        for (id, cert_path, key_path) in paths {
            let cert = fs::read(cert_path)?;
            let key = fs::read(key_path)?;
            certificates.push(CertMaterial {
                id,
                cert: base64::engine::general_purpose::STANDARD.encode(cert),
                key: base64::engine::general_purpose::STANDARD.encode(key),
            });
        }
        let bundle = RecoveryBundle {
            version: 1,
            preview: preview.clone(),
            database: base64::engine::general_purpose::STANDARD.encode(fs::read(&snapshot_path)?),
            certificates,
        };
        let encrypted = encrypt_bundle(&bundle, passphrase)?;
        let mut output = secure_new_file(save_path)?;
        if let Err(error) = output.write_all(&encrypted) {
            drop(output);
            let _ = fs::remove_file(save_path);
            return Err(AppError::Io(error));
        }
        if let Err(error) = output.sync_all() {
            drop(output);
            let _ = fs::remove_file(save_path);
            return Err(AppError::Io(error));
        }
        Ok(preview)
    })();
    let _ = fs::remove_file(&snapshot_path);
    result
}

pub fn preview_bundle(path: &Path, passphrase: &str) -> Result<RecoveryPreview, AppError> {
    Ok(read_bundle(path, passphrase)?.preview)
}

/// Tables a restore replaces, in row-copy order: referenced rows land before the
/// rows pointing at them, so deletions run in the reverse order.
/// Tables a restore deliberately does not copy yet, each waiting on its own
/// ticket. They stay listed so the gap is visible: an unlisted, unrestored table
/// means a restored bundle silently mixes with leftover local rows.
#[cfg_attr(not(test), allow(dead_code))]
const RESTORE_DEFERRED_TABLES: [&str; 1] = ["hostname_settings"];

const RESTORE_TABLES: [&str; 8] = [
    "dns_credentials",
    "acme_accounts",
    "access_lists",
    "certificates",
    "access_rules",
    "proxy_rules",
    "host_entries",
    "app_settings",
];

/// Quote an SQL identifier for interpolation. Values are never interpolated.
fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Column names of `table`, taken from the connection's main schema or from the
/// attached `restored` schema when `schema` is given.
fn table_columns(
    conn: &Connection,
    schema: Option<&str>,
    table: &str,
) -> Result<Vec<String>, AppError> {
    let pragma = match schema {
        Some(schema) => format!(
            "PRAGMA {}.table_info({})",
            quote_identifier(schema),
            quote_identifier(table)
        ),
        None => format!("PRAGMA table_info({})", quote_identifier(table)),
    };
    let mut statement = conn.prepare(&pragma)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

/// Replace every restored table's rows with the rows from the attached
/// `restored` database.
///
/// Rows move column by column through the intersection of the bundled database's
/// columns and the current table definition, which is the one place that decides
/// what a future schema change does to a restore: a column added to the schema
/// after a bundle was written is absent from the bundle, so it is left out of the
/// `INSERT` and keeps its schema default. A column removed from the schema is
/// likewise left behind in the bundle.
fn replace_tables_from_restored(conn: &Connection) -> Result<(), AppError> {
    for table in RESTORE_TABLES.iter().rev() {
        conn.execute(&format!("DELETE FROM {}", quote_identifier(table)), [])?;
    }
    for table in RESTORE_TABLES {
        let current_columns = table_columns(conn, None, table)?;
        let bundled_columns = table_columns(conn, Some("restored"), table)?;
        if bundled_columns.is_empty() {
            return Err(recovery_error(&format!(
                "Recovery database is missing table {}",
                table
            )));
        }
        let copied_columns = current_columns
            .iter()
            .filter(|column| bundled_columns.contains(*column))
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>();
        if copied_columns.is_empty() {
            return Err(recovery_error(&format!(
                "Recovery database has no columns in common with table {}",
                table
            )));
        }
        let columns = copied_columns.join(", ");
        let table = quote_identifier(table);
        conn.execute(
            &format!("INSERT INTO {table} ({columns}) SELECT {columns} FROM \"restored\".{table}"),
            [],
        )?;
    }
    Ok(())
}

pub fn restore_bundle(
    db: &mut Connection,
    db_path: &Path,
    data_dir: &Path,
    path: &Path,
    passphrase: &str,
) -> Result<String, AppError> {
    let bundle = read_bundle(path, passphrase)?;
    let database = base64::engine::general_purpose::STANDARD
        .decode(&bundle.database)
        .map_err(|_| recovery_error("Recovery database is damaged"))?;
    if !database.starts_with(b"SQLite format 3\0") || database.len() as u64 > MAX_BUNDLE_BYTES {
        return Err(recovery_error("Recovery database is invalid or oversized"));
    }

    let temporary_path = data_dir.join(format!("recovery-import-{}.db", uuid::Uuid::new_v4()));
    let mut created_files: Vec<PathBuf> = Vec::new();
    let mut attached = false;
    let mut config_applied = false;
    let result = (|| -> Result<String, AppError> {
        let mut temporary = secure_new_file(&temporary_path)?;
        temporary.write_all(&database)?;
        temporary.sync_all()?;
        drop(temporary);

        let source = Connection::open(&temporary_path)?;
        let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(recovery_error("Recovery database failed integrity check"));
        }
        let ready_ids: HashSet<String> = source
            .prepare("SELECT id FROM certificates WHERE status = 'ready'")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        let material_ids: HashSet<String> =
            bundle.certificates.iter().map(|c| c.id.clone()).collect();
        if ready_ids != material_ids || bundle.certificates.len() != material_ids.len() {
            return Err(recovery_error(
                "Recovery certificate files do not match database records",
            ));
        }
        if count(&source, "proxy_rules")? != bundle.preview.proxy_count
            || count(&source, "certificates")? != bundle.preview.certificate_count
            || count(&source, "host_entries")? != bundle.preview.host_count
            || count(&source, "dns_credentials")? != bundle.preview.dns_credential_count
        {
            return Err(recovery_error("Recovery manifest does not match database"));
        }
        drop(source);

        let backup = store::backup_database(db_path)?;
        let temporary_path_str = temporary_path.to_string_lossy();
        db.execute(
            "ATTACH DATABASE ?1 AS restored",
            [temporary_path_str.as_ref()],
        )?;
        attached = true;

        let certs_dir = data_dir.join("nginx").join("certs");
        fs::create_dir_all(&certs_dir)?;
        let mut rewritten_paths = Vec::with_capacity(bundle.certificates.len());
        for material in &bundle.certificates {
            let cert_bytes = base64::engine::general_purpose::STANDARD
                .decode(&material.cert)
                .map_err(|_| recovery_error("Recovery certificate is damaged"))?;
            let key_bytes = base64::engine::general_purpose::STANDARD
                .decode(&material.key)
                .map_err(|_| recovery_error("Recovery private key is damaged"))?;
            if cert_bytes.len() > 1024 * 1024
                || key_bytes.len() > 1024 * 1024
                || !cert_bytes.starts_with(b"-----BEGIN CERTIFICATE-----")
                || !key_bytes.starts_with(b"-----BEGIN")
            {
                return Err(recovery_error("Recovery certificate material is invalid"));
            }
            let id = uuid::Uuid::new_v4();
            let cert_path = certs_dir.join(format!("{}.cert.pem", id));
            let key_path = certs_dir.join(format!("{}.key.pem", id));
            let mut cert_file = secure_new_file(&cert_path)?;
            created_files.push(cert_path.clone());
            cert_file.write_all(&cert_bytes)?;
            cert_file.sync_all()?;
            let mut key_file = secure_new_file(&key_path)?;
            created_files.push(key_path.clone());
            key_file.write_all(&key_bytes)?;
            key_file.sync_all()?;
            rewritten_paths.push((material.id.clone(), cert_path, key_path));
        }

        let tx = db.transaction()?;
        replace_tables_from_restored(&tx)?;
        for (id, cert_path, key_path) in &rewritten_paths {
            tx.execute(
                "UPDATE certificates SET cert_path = ?1, key_path = ?2 WHERE id = ?3",
                params![cert_path.to_string_lossy(), key_path.to_string_lossy(), id],
            )?;
        }
        tx.execute(
            "INSERT INTO app_settings(key, value) VALUES ('hosts_sync_status', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::json!({
                "synced": false,
                "checked_at": chrono::Utc::now().to_rfc3339(),
                "error": "Restored hosts entries need an explicit system sync"
            })
            .to_string()],
        )?;
        config_engine::apply_db_state(&tx, data_dir)?;
        config_applied = true;
        tx.commit()?;
        Ok(backup)
    })();

    if attached {
        let _ = db.execute_batch("DETACH DATABASE restored");
    }
    let _ = fs::remove_file(&temporary_path);
    if result.is_err() {
        if config_applied {
            let _ = config_engine::apply_db_state(db, data_dir);
        }
        for file in created_files {
            let _ = fs::remove_file(file);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSPHRASE: &str = "correct horse battery staple";

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("meridian-{}-{}", prefix, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The full restore path drives the nginx config engine, so it only runs
    /// where a sidecar sits next to the test binary. The column copy itself is
    /// covered by the tests that attach the bundled database directly.
    fn nginx_sidecar_available() -> bool {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|dir| dir.join(if cfg!(windows) { "nginx.exe" } else { "nginx" }))
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    /// Create a database whose `proxy_rules` table predates the
    /// `upstream_scheme` column, like a bundle exported by an older build.
    fn legacy_database(path: &Path) {
        let db = store::init_database(path).unwrap();
        db.execute_batch(
            "ALTER TABLE proxy_rules DROP COLUMN upstream_scheme;
             INSERT INTO proxy_rules
                 (id, name, proxy_type, listen_port, upstream_host, upstream_port, created_at, updated_at)
             VALUES
                 ('legacy-rule', 'legacy rule', 'http', 8080, '127.0.0.1', 9000, 'now', 'now');",
        )
        .unwrap();
    }

    /// Decode the database a bundle carries into a standalone file.
    fn unpack_bundle_database(bundle_path: &Path, dir: &Path) -> PathBuf {
        let bundle = read_bundle(bundle_path, PASSPHRASE).unwrap();
        let import_path = dir.join("import.db");
        fs::write(
            &import_path,
            base64::engine::general_purpose::STANDARD
                .decode(&bundle.database)
                .unwrap(),
        )
        .unwrap();
        import_path
    }

    /// Attach an unpacked bundle and run the restore copy into `live`.
    fn copy_bundle_into(live: &Connection, import_path: &Path) {
        let import_path_str = import_path.to_string_lossy();
        live.execute("ATTACH DATABASE ?1 AS restored", [import_path_str.as_ref()])
            .unwrap();
        replace_tables_from_restored(live).unwrap();
        live.execute_batch("DETACH DATABASE restored").unwrap();
    }

    #[test]
    fn encrypted_bundle_roundtrip_rejects_wrong_passphrase_and_tampering() {
        let dir =
            std::env::temp_dir().join(format!("meridian-recovery-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("meridian.db");
        let db = store::init_database(&db_path).unwrap();
        db.execute("INSERT INTO host_entries (id, ip, hostname, created_at, updated_at) VALUES ('one', '127.0.0.1', 'local.test', 'now', 'now')", []).unwrap();
        drop(db);
        let bundle_path = dir.join("backup.meridian");
        let expected = create_bundle(&db_path, &bundle_path, PASSPHRASE).unwrap();
        assert_eq!(expected.host_count, 1);
        let file = fs::read(&bundle_path).unwrap();
        assert!(file.starts_with(MAGIC));
        assert!(!file.windows("local.test".len()).any(|w| w == b"local.test"));
        assert!(preview_bundle(&bundle_path, "wrong password long enough").is_err());
        assert_eq!(
            preview_bundle(&bundle_path, PASSPHRASE).unwrap().host_count,
            1
        );
        let mut tampered = file;
        let final_byte = tampered.last_mut().unwrap();
        *final_byte ^= 1;
        fs::write(&bundle_path, tampered).unwrap();
        assert!(preview_bundle(&bundle_path, PASSPHRASE).is_err());
        let _ = fs::remove_file(bundle_path);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn restore_bundle_replaces_database_and_keeps_previous_backup() {
        if !nginx_sidecar_available() {
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("meridian-restore-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("meridian.db");
        let mut db = store::init_database(&db_path).unwrap();
        db.execute("INSERT INTO host_entries (id, ip, hostname, created_at, updated_at) VALUES ('original', '127.0.0.1', 'original.test', 'now', 'now')", []).unwrap();
        let material =
            crate::cert_manager::generate_self_signed(&dir, "test", "original.test", 30).unwrap();
        let original_cert = crate::store::cert_repo::create(&db, &material).unwrap();
        let original_key = fs::read(&original_cert.key_path).unwrap();
        let bundle_path = dir.join("backup.meridian");
        create_bundle(&db_path, &bundle_path, PASSPHRASE).unwrap();
        db.execute("DELETE FROM host_entries", []).unwrap();
        db.execute("INSERT INTO host_entries (id, ip, hostname, created_at, updated_at) VALUES ('new', '127.0.0.1', 'new.test', 'now', 'now')", []).unwrap();
        let previous_db =
            restore_bundle(&mut db, &db_path, &dir, &bundle_path, PASSPHRASE).unwrap();
        let restored: String = db
            .query_row("SELECT hostname FROM host_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(restored, "original.test");
        let restored_cert = crate::store::cert_repo::get_by_id(&db, &original_cert.id).unwrap();
        assert_ne!(restored_cert.key_path, original_cert.key_path);
        assert_eq!(fs::read(&restored_cert.key_path).unwrap(), original_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&restored_cert.key_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let previous = Connection::open(&previous_db).unwrap();
        let old: String = previous
            .query_row("SELECT hostname FROM host_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(old, "new.test");
        let sync_status: String = db
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'hosts_sync_status'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sync_status.contains("Restored hosts entries"));
        drop(previous);
        drop(db);
        for file in [
            &original_cert.cert_path,
            &original_cert.key_path,
            &restored_cert.cert_path,
            &restored_cert.key_path,
        ] {
            let _ = fs::remove_file(file);
        }
        let _ = fs::remove_file(previous_db);
        let _ = fs::remove_file(bundle_path);
        let _ = fs::remove_file(db_path);
        for file in [
            "nginx/nginx.conf",
            "nginx/html/502.html",
            "nginx/logs/error.log",
        ] {
            let _ = fs::remove_file(dir.join(file));
        }
        for subdir in [
            "nginx/conf.d",
            "nginx/stream.d",
            "nginx/certs",
            "nginx/html",
            "nginx/logs",
            "nginx/temp/client_body",
            "nginx/temp/proxy",
            "nginx/temp",
            "nginx",
            "",
        ] {
            let _ = fs::remove_dir(dir.join(subdir));
        }
    }

    #[test]
    fn restore_copy_moves_every_current_column() {
        let dir = temp_dir("recovery-current-columns");
        let source_path = dir.join("source.db");
        let source = store::init_database(&source_path).unwrap();
        source
            .execute_batch(
                "INSERT INTO dns_credentials (id, name, provider, credentials_json, created_at, updated_at)
                 VALUES ('dns-one', 'main', 'cloudflare', '{\"token\":\"secret\"}', 'now', 'now');
                 INSERT INTO proxy_rules
                     (id, name, proxy_type, enabled, listen_port, listen_host, domain, path_prefix,
                      upstream_host, upstream_port, tls_mode, websocket, keep_alive, custom_headers,
                      sort_order, created_at, updated_at, upstream_targets, upstream_scheme)
                 VALUES
                     ('rule-one', 'demo', 'http', 1, 8080, '127.0.0.1', 'demo.test', '/',
                      '127.0.0.1', 9000, 'none', 1, 1, 'X-Demo: yes',
                      3, 'now', 'now', '[{\"host\":\"10.0.0.2\",\"port\":9000}]', 'https');
                 INSERT INTO host_entries (id, ip, hostname, comment, enabled, created_at, updated_at)
                 VALUES ('host-one', '127.0.0.1', 'demo.test', 'keep me', 1, 'now', 'now');
                 INSERT INTO app_settings (key, value) VALUES ('worker_processes', '4');",
            )
            .unwrap();
        drop(source);
        let bundle_path = dir.join("current.meridian");
        create_bundle(&source_path, &bundle_path, PASSPHRASE).unwrap();
        let import_path = unpack_bundle_database(&bundle_path, &dir);

        let live_path = dir.join("live.db");
        let live = store::init_database(&live_path).unwrap();
        copy_bundle_into(&live, &import_path);

        let (name, scheme, targets, keep_alive): (String, String, String, i64) = live
            .query_row(
                "SELECT name, upstream_scheme, COALESCE(upstream_targets, ''), keep_alive
                 FROM proxy_rules",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(name, "demo");
        assert_eq!(scheme, "https");
        assert_eq!(targets, r#"[{"host":"10.0.0.2","port":9000}]"#);
        assert_eq!(keep_alive, 1);
        let (comment, enabled): (Option<String>, i64) = live
            .query_row(
                "SELECT comment, enabled FROM host_entries WHERE id = 'host-one'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(comment.as_deref(), Some("keep me"));
        assert_eq!(enabled, 1);
        let credentials: String = live
            .query_row(
                "SELECT credentials_json FROM dns_credentials WHERE id = 'dns-one'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(credentials, r#"{"token":"secret"}"#);
        let worker_processes: String = live
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'worker_processes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(worker_processes, "4");
        drop(live);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_copy_defaults_columns_absent_from_the_bundle() {
        let dir = temp_dir("recovery-missing-column");
        let legacy_path = dir.join("legacy.db");
        legacy_database(&legacy_path);
        let bundle_path = dir.join("legacy.meridian");
        create_bundle(&legacy_path, &bundle_path, PASSPHRASE).unwrap();
        let import_path = unpack_bundle_database(&bundle_path, &dir);

        let live_path = dir.join("live.db");
        let live = store::init_database(&live_path).unwrap();
        copy_bundle_into(&live, &import_path);

        let (name, scheme, listen_port): (String, String, i64) = live
            .query_row(
                "SELECT name, upstream_scheme, listen_port FROM proxy_rules",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "legacy rule");
        assert_eq!(listen_port, 8080);
        assert_eq!(scheme, "http"); // Schema default for the column the bundle predates.
        drop(live);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_bundle_accepts_a_bundle_missing_a_later_column() {
        if !nginx_sidecar_available() {
            return;
        }
        let dir = temp_dir("recovery-missing-column-restore");
        let legacy_path = dir.join("legacy.db");
        legacy_database(&legacy_path);
        let bundle_path = dir.join("legacy.meridian");
        create_bundle(&legacy_path, &bundle_path, PASSPHRASE).unwrap();

        let live_path = dir.join("live.db");
        let mut live = store::init_database(&live_path).unwrap();
        restore_bundle(&mut live, &live_path, &dir, &bundle_path, PASSPHRASE).unwrap();
        let (name, scheme): (String, String) = live
            .query_row("SELECT name, upstream_scheme FROM proxy_rules", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(name, "legacy rule");
        assert_eq!(scheme, "http");
        drop(live);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_bundle_rejects_certificate_material_that_does_not_match_the_database() {
        let dir = temp_dir("recovery-certificate-mismatch");
        let db_path = dir.join("meridian.db");
        let mut db = store::init_database(&db_path).unwrap();
        let snapshot_path = store::backup_database(&db_path).unwrap();
        let database = fs::read(&snapshot_path).unwrap();
        let _ = fs::remove_file(&snapshot_path);
        let bundle = RecoveryBundle {
            version: 1,
            preview: RecoveryPreview {
                exported_at: chrono::Utc::now().to_rfc3339(),
                proxy_count: 0,
                certificate_count: 0,
                host_count: 0,
                dns_credential_count: 0,
            },
            database: base64::engine::general_purpose::STANDARD.encode(database),
            certificates: vec![CertMaterial {
                id: "missing-from-database".to_string(),
                cert: base64::engine::general_purpose::STANDARD
                    .encode("-----BEGIN CERTIFICATE-----\n"),
                key: base64::engine::general_purpose::STANDARD
                    .encode("-----BEGIN PRIVATE KEY-----\n"),
            }],
        };
        let bundle_path = dir.join("mismatch.meridian");
        fs::write(&bundle_path, encrypt_bundle(&bundle, PASSPHRASE).unwrap()).unwrap();

        let error = restore_bundle(&mut db, &db_path, &dir, &bundle_path, PASSPHRASE)
            .expect_err("certificate material must match the bundled database");
        assert!(
            matches!(
                &error,
                AppError::Validation(message)
                    if message == "Recovery certificate files do not match database records"
            ),
            "unexpected error: {error}"
        );
        drop(db);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_schema_table_is_restored_or_explicitly_deferred() {
        let dir = temp_dir("recovery-schema-tables");
        let db_path = dir.join("meridian.db");
        let db = store::init_database(&db_path).unwrap();
        let tables: Vec<String> = {
            let mut statement = db
                .prepare(
                    "SELECT name FROM sqlite_master
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
                     ORDER BY name",
                )
                .unwrap();
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        };
        drop(db);

        let unaccounted: Vec<&str> = tables
            .iter()
            .map(String::as_str)
            .filter(|table| {
                !RESTORE_TABLES.contains(table) && !RESTORE_DEFERRED_TABLES.contains(table)
            })
            .collect();

        let _ = fs::remove_dir_all(&dir);

        assert!(
            unaccounted.is_empty(),
            "schema tables are neither restored nor explicitly deferred: {unaccounted:?}; \
             add them to RESTORE_TABLES or to RESTORE_DEFERRED_TABLES with the ticket that will"
        );
    }
}
