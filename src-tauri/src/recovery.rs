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
        tx.execute_batch(
            "DELETE FROM access_rules;
             DELETE FROM proxy_rules;
             DELETE FROM certificates;
             DELETE FROM access_lists;
             DELETE FROM dns_credentials;
             DELETE FROM acme_accounts;
             DELETE FROM host_entries;
             DELETE FROM app_settings;
             INSERT INTO dns_credentials SELECT * FROM restored.dns_credentials;
             INSERT INTO acme_accounts SELECT * FROM restored.acme_accounts;
             INSERT INTO access_lists SELECT * FROM restored.access_lists;
             INSERT INTO certificates SELECT * FROM restored.certificates;
             INSERT INTO access_rules SELECT * FROM restored.access_rules;
             INSERT INTO proxy_rules SELECT * FROM restored.proxy_rules;
             INSERT INTO host_entries SELECT * FROM restored.host_entries;
             INSERT INTO app_settings SELECT * FROM restored.app_settings;",
        )?;
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
        let expected =
            create_bundle(&db_path, &bundle_path, "correct horse battery staple").unwrap();
        assert_eq!(expected.host_count, 1);
        let file = fs::read(&bundle_path).unwrap();
        assert!(file.starts_with(MAGIC));
        assert!(!file.windows("local.test".len()).any(|w| w == b"local.test"));
        assert!(preview_bundle(&bundle_path, "wrong password long enough").is_err());
        assert_eq!(
            preview_bundle(&bundle_path, "correct horse battery staple")
                .unwrap()
                .host_count,
            1
        );
        let mut tampered = file;
        let final_byte = tampered.last_mut().unwrap();
        *final_byte ^= 1;
        fs::write(&bundle_path, tampered).unwrap();
        assert!(preview_bundle(&bundle_path, "correct horse battery staple").is_err());
        let _ = fs::remove_file(bundle_path);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn restore_bundle_replaces_database_and_keeps_previous_backup() {
        if !std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join(if cfg!(windows) { "nginx.exe" } else { "nginx" })
            .exists()
        {
            return; // The full restore check runs when a local Nginx sidecar is available.
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
        create_bundle(&db_path, &bundle_path, "correct horse battery staple").unwrap();
        db.execute("DELETE FROM host_entries", []).unwrap();
        db.execute("INSERT INTO host_entries (id, ip, hostname, created_at, updated_at) VALUES ('new', '127.0.0.1', 'new.test', 'now', 'now')", []).unwrap();
        let previous_db = restore_bundle(
            &mut db,
            &db_path,
            &dir,
            &bundle_path,
            "correct horse battery staple",
        )
        .unwrap();
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
}
