pub mod conflict;
pub mod error_pages;
pub mod http_config;
pub mod main_config;
pub mod stream_config;

use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Convert a path to a string with forward slashes.
/// nginx on all platforms (including Windows) accepts forward slashes.
pub fn nginx_path(p: &Path) -> Cow<'_, str> {
    let s = p.to_string_lossy();
    if s.contains('\\') {
        Cow::Owned(s.replace('\\', "/"))
    } else {
        s
    }
}

/// Normalize a path string (e.g. cert_path from database) to forward slashes for nginx.
pub fn nginx_path_str(s: &str) -> Cow<'_, str> {
    if s.contains('\\') {
        Cow::Owned(s.replace('\\', "/"))
    } else {
        Cow::Borrowed(s)
    }
}

fn write_config(path: &Path, content: &str) -> Result<(), AppError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.set_len(0)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

use tracing::{info, warn};

use crate::error::AppError;
use crate::store::models::{AccessList, AccessRule, Certificate, PortConflict, ProxyRule};
use crate::store::{access_repo, cert_repo, proxy_repo, settings_repo};

pub struct ConfigData {
    pub rules: Vec<ProxyRule>,
    pub certs: Vec<Certificate>,
    pub access_lists: Vec<(AccessList, Vec<AccessRule>)>,
    pub worker_processes: String,
}

#[derive(Serialize)]
pub struct ConfigChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Serialize)]
pub struct ConfigPreview {
    pub valid: bool,
    pub test_message: String,
    pub conflicts: Vec<PortConflict>,
    pub changes: Vec<ConfigChange>,
}

fn generated_files(data_dir: &Path) -> Result<HashMap<String, String>, AppError> {
    let mut files = HashMap::new();
    let nginx_dir = data_dir.join("nginx");
    for relative_dir in ["", "conf.d", "stream.d"] {
        let dir = nginx_dir.join(relative_dir);
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "conf") {
                continue;
            }
            let relative = path
                .strip_prefix(&nginx_dir)
                .map_err(|e| AppError::Config(e.to_string()))?;
            files.insert(
                relative.to_string_lossy().replace('\\', "/"),
                fs::read_to_string(path)?,
            );
        }
    }
    Ok(files)
}

fn cleanup_preview_dir(dir: &Path) {
    for relative in [
        "nginx/conf.d",
        "nginx/stream.d",
        "nginx/html",
        "nginx/logs",
        "nginx/temp/client_body",
        "nginx/temp/proxy",
        "nginx/temp",
        "nginx",
    ] {
        let path = dir.join(relative);
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let _ = fs::remove_dir(path);
    }
    let _ = fs::remove_dir(dir);
}

pub fn preview_db_state(
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<ConfigPreview, AppError> {
    let data = load_config_data(db)?;
    let staged = data_dir.join(format!("nginx_preview_{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<ConfigPreview, AppError> {
        let conflicts = generate_all_configs_with_settings(
            &staged,
            &data.rules,
            &data.certs,
            &data.access_lists,
            &data.worker_processes,
        )?;
        let staged_files = generated_files(&staged)?;
        let live_files = generated_files(data_dir)?;
        let staged_prefix = nginx_path(&staged).into_owned();
        let live_prefix = nginx_path(data_dir).into_owned();
        let mut names: Vec<String> = staged_files
            .keys()
            .chain(live_files.keys())
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        let changes = names
            .into_iter()
            .filter_map(|path| {
                let before = live_files.get(&path).cloned();
                let after = staged_files
                    .get(&path)
                    .map(|s| s.replace(&staged_prefix, &live_prefix));
                if before == after {
                    None
                } else {
                    Some(ConfigChange {
                        path,
                        before,
                        after,
                    })
                }
            })
            .collect();
        let (valid, test_message) = if conflicts.is_empty() {
            match crate::nginx_manager::test_config(&staged) {
                Ok(result) => result,
                Err(error) => (false, error.to_string()),
            }
        } else {
            (
                false,
                conflicts
                    .iter()
                    .map(|c| c.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        };
        Ok(ConfigPreview {
            valid,
            test_message,
            conflicts,
            changes,
        })
    })();
    cleanup_preview_dir(&staged);
    result
}

pub fn load_config_data(db: &rusqlite::Connection) -> Result<ConfigData, AppError> {
    let rules = proxy_repo::list_enabled(db)?;
    let certs = cert_repo::list_all(db)?;
    let lists = access_repo::list_all_lists(db)?;
    let mut access_lists = Vec::with_capacity(lists.len());
    for list in lists {
        let rules = access_repo::list_rules_by_list(db, &list.id)?;
        access_lists.push((list, rules));
    }
    let worker_processes =
        settings_repo::get(db, "worker_processes")?.unwrap_or_else(|| "2".to_string());
    Ok(ConfigData {
        rules,
        certs,
        access_lists,
        worker_processes,
    })
}

pub fn apply_db_state(
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<Vec<PortConflict>, AppError> {
    let data = load_config_data(db)?;
    apply_and_reload(
        data_dir,
        &data.rules,
        &data.certs,
        &data.access_lists,
        &data.worker_processes,
    )
}

/// Generate all configs with explicit settings.
/// `worker_processes`: "auto" or a numeric string (default "2").
pub fn generate_all_configs_with_settings(
    data_dir: &Path,
    rules: &[ProxyRule],
    certs: &[Certificate],
    access_lists: &[(AccessList, Vec<AccessRule>)],
    worker_processes: &str,
) -> Result<Vec<PortConflict>, AppError> {
    let nginx_dir = data_dir.join("nginx");
    let conf_d = nginx_dir.join("conf.d");
    let stream_d = nginx_dir.join("stream.d");
    let logs_dir = nginx_dir.join("logs");

    // Ensure directories exist
    fs::create_dir_all(&conf_d)?;
    fs::create_dir_all(&stream_d)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(nginx_dir.join("temp"))?;

    // Write custom error pages
    error_pages::write_error_pages(data_dir)?;

    // Detect conflicts
    let conflicts = conflict::detect_conflicts(rules);

    // Write main nginx.conf
    let main_conf = main_config::generate_main_config(data_dir, worker_processes);
    write_config(&nginx_dir.join("nginx.conf"), &main_conf)?;
    info!("Wrote nginx.conf");

    // Clear existing generated configs
    clear_directory(&conf_d)?;
    clear_directory(&stream_d)?;

    // Separate HTTP and stream rules
    let http_rules: Vec<&ProxyRule> = rules.iter().filter(|r| r.proxy_type == "http").collect();
    let stream_rules: Vec<&ProxyRule> = rules
        .iter()
        .filter(|r| r.proxy_type == "stream_tcp" || r.proxy_type == "stream_udp")
        .collect();

    // Group HTTP rules by (listen_port, domain) for virtual hosting
    let mut http_groups: HashMap<(u16, String), Vec<&ProxyRule>> = HashMap::new();
    for rule in &http_rules {
        let domain = rule.domain.clone().unwrap_or_default();
        http_groups
            .entry((rule.listen_port, domain))
            .or_default()
            .push(rule);
    }

    // Generate HTTP config files
    for ((port, domain), group_rules) in &http_groups {
        let config = http_config::generate_server_block(group_rules, certs, access_lists, data_dir);
        let sanitized_domain = if domain.is_empty() {
            "default".to_string()
        } else {
            domain.replace('.', "_").replace('*', "wildcard")
        };
        let filename = format!("{}_{}.conf", port, sanitized_domain);
        write_config(&conf_d.join(&filename), &config)?;
        info!("Wrote HTTP config: {}", filename);
    }

    // Generate stream config files
    for rule in &stream_rules {
        let config = stream_config::generate_stream_block(rule, certs, data_dir);
        let filename = format!("stream_{}_{}.conf", rule.listen_port, rule.proxy_type);
        write_config(&stream_d.join(&filename), &config)?;
        info!("Wrote stream config: {}", filename);
    }

    Ok(conflicts)
}

fn clear_directory(dir: &Path) -> Result<(), AppError> {
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "conf") {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

/// Backup conf.d/ and stream.d/ directories to a temporary location.
/// Returns the backup directory path.
pub fn backup_configs(data_dir: &Path) -> Result<PathBuf, AppError> {
    let nginx_dir = data_dir.join("nginx");
    let backup_dir = data_dir.join(format!("nginx_backup_{}", uuid::Uuid::new_v4()));

    fs::create_dir_all(&backup_dir)?;

    let conf_d = nginx_dir.join("conf.d");
    let stream_d = nginx_dir.join("stream.d");
    let backup_conf_d = backup_dir.join("conf.d");
    let backup_stream_d = backup_dir.join("stream.d");

    copy_dir_conf_files(&conf_d, &backup_conf_d)?;
    copy_dir_conf_files(&stream_d, &backup_stream_d)?;

    // Also backup nginx.conf
    let nginx_conf = nginx_dir.join("nginx.conf");
    if nginx_conf.exists() {
        fs::copy(&nginx_conf, backup_dir.join("nginx.conf"))?;
    }

    info!("Backed up nginx configs to {:?}", backup_dir);
    Ok(backup_dir)
}

/// Restore configs from backup directory.
pub fn restore_configs(backup_dir: &Path, data_dir: &Path) -> Result<(), AppError> {
    let nginx_dir = data_dir.join("nginx");

    let conf_d = nginx_dir.join("conf.d");
    let stream_d = nginx_dir.join("stream.d");

    // Clear current configs
    clear_directory(&conf_d)?;
    clear_directory(&stream_d)?;

    // Restore from backup
    let backup_conf_d = backup_dir.join("conf.d");
    let backup_stream_d = backup_dir.join("stream.d");

    copy_dir_conf_files(&backup_conf_d, &conf_d)?;
    copy_dir_conf_files(&backup_stream_d, &stream_d)?;

    // Restore nginx.conf
    let backup_nginx_conf = backup_dir.join("nginx.conf");
    if backup_nginx_conf.exists() {
        fs::copy(&backup_nginx_conf, nginx_dir.join("nginx.conf"))?;
    }

    // Cleanup backup
    let _ = cleanup_backup_dir(backup_dir);

    info!("Restored nginx configs from backup");
    Ok(())
}

fn copy_dir_conf_files(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dst)?;
    if src.exists() {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let dest_file = dst.join(entry.file_name());
                fs::copy(&path, &dest_file)?;
            }
        }
    }
    Ok(())
}

fn cleanup_backup_dir(backup_dir: &Path) -> Result<(), AppError> {
    let backup_conf_d = backup_dir.join("conf.d");
    let backup_stream_d = backup_dir.join("stream.d");
    clear_directory(&backup_conf_d)?;
    clear_directory(&backup_stream_d)?;

    let backup_nginx_conf = backup_dir.join("nginx.conf");
    if backup_nginx_conf.exists() && backup_nginx_conf.is_file() {
        fs::remove_file(backup_nginx_conf)?;
    }

    if backup_conf_d.exists() {
        let _ = fs::remove_dir(&backup_conf_d);
    }
    if backup_stream_d.exists() {
        let _ = fs::remove_dir(&backup_stream_d);
    }
    if backup_dir.exists() {
        let _ = fs::remove_dir(backup_dir);
    }
    Ok(())
}

/// Apply config generation with backup, test, and optional reload.
/// Returns conflicts detected during generation.
/// On test failure, restores the backup and returns an error.
pub fn apply_and_reload(
    data_dir: &Path,
    rules: &[ProxyRule],
    certs: &[Certificate],
    access_lists: &[(AccessList, Vec<AccessRule>)],
    worker_processes: &str,
) -> Result<Vec<PortConflict>, AppError> {
    // Step 1: backup existing configs
    let backup_dir = backup_configs(data_dir)?;

    // Step 2: generate new configs
    let conflicts = match generate_all_configs_with_settings(
        data_dir,
        rules,
        certs,
        access_lists,
        worker_processes,
    ) {
        Ok(c) => c,
        Err(e) => {
            warn!("Config generation failed, restoring backup: {}", e);
            let _ = restore_configs(&backup_dir, data_dir);
            return Err(e);
        }
    };
    if !conflicts.is_empty() {
        let message = conflicts
            .iter()
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        warn!("Config conflict detected, restoring backup: {}", message);
        let _ = restore_configs(&backup_dir, data_dir);
        return Err(AppError::Conflict(message));
    }

    // Step 3: test nginx config
    match crate::nginx_manager::test_config(data_dir) {
        Ok((true, _)) => {
            // Step 4: reload if nginx is running
            let status = crate::nginx_manager::status(data_dir);
            if status.status == "running" {
                if let Err(e) = crate::nginx_manager::reload(data_dir) {
                    warn!("Reload failed, restoring backup: {}", e);
                    let _ = restore_configs(&backup_dir, data_dir);
                    return Err(e);
                }
            }
            // Cleanup backup on success
            let _ = cleanup_backup_dir(&backup_dir);
            Ok(conflicts)
        }
        Ok((false, error_msg)) => {
            warn!("Config test failed, restoring backup: {}", error_msg);
            let _ = restore_configs(&backup_dir, data_dir);
            Err(AppError::Config(format!(
                "nginx config test failed: {}",
                error_msg
            )))
        }
        Err(e) => {
            warn!("Could not test config, restoring backup: {}", e);
            let _ = restore_configs(&backup_dir, data_dir);
            Err(e)
        }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn generated_config_is_private_on_unix() {
        let path =
            std::env::temp_dir().join(format!("meridian-config-mode-{}", uuid::Uuid::new_v4()));
        write_config(&path, "worker_processes 2;").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn preview_leaves_live_configuration_unchanged() {
        let data_dir =
            std::env::temp_dir().join(format!("meridian-preview-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&data_dir).unwrap();
        let db_path = data_dir.join("meridian.db");
        let db = crate::store::init_database(&db_path).unwrap();
        let preview = preview_db_state(&db, &data_dir).unwrap();
        assert!(preview
            .changes
            .iter()
            .any(|change| change.path == "nginx.conf"));
        if std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join(if cfg!(windows) { "nginx.exe" } else { "nginx" })
            .exists()
        {
            assert!(preview.valid, "{}", preview.test_message);
        }
        assert!(!data_dir.join("nginx/nginx.conf").exists());
        assert!(
            !fs::read_dir(&data_dir).unwrap().flatten().any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("nginx_preview_"))
        );
        drop(db);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_dir(&data_dir);
    }
}
