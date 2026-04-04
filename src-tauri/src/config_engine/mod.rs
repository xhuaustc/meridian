pub mod conflict;
pub mod http_config;
pub mod main_config;
pub mod stream_config;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::error::AppError;
use crate::store::models::{AccessList, AccessRule, Certificate, PortConflict, ProxyRule};

/// Orchestrate full nginx config generation from all enabled rules.
/// Writes config files to disk and returns any conflicts detected.
pub fn generate_all_configs(
    data_dir: &Path,
    rules: &[ProxyRule],
    certs: &[Certificate],
    access_lists: &[(AccessList, Vec<AccessRule>)],
) -> Result<Vec<PortConflict>, AppError> {
    let nginx_dir = data_dir.join("nginx");
    let conf_d = nginx_dir.join("conf.d");
    let stream_d = nginx_dir.join("stream.d");
    let logs_dir = nginx_dir.join("logs");

    // Ensure directories exist
    fs::create_dir_all(&conf_d)?;
    fs::create_dir_all(&stream_d)?;
    fs::create_dir_all(&logs_dir)?;

    // Detect conflicts
    let conflicts = conflict::detect_conflicts(rules);

    // Write main nginx.conf
    let main_conf = main_config::generate_main_config(data_dir);
    fs::write(nginx_dir.join("nginx.conf"), main_conf)?;
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
        let config = http_config::generate_server_block(group_rules, certs, access_lists);
        let sanitized_domain = if domain.is_empty() {
            "default".to_string()
        } else {
            domain.replace('.', "_").replace('*', "wildcard")
        };
        let filename = format!("{}_{}.conf", port, sanitized_domain);
        fs::write(conf_d.join(&filename), config)?;
        info!("Wrote HTTP config: {}", filename);
    }

    // Generate stream config files
    for rule in &stream_rules {
        let config = stream_config::generate_stream_block(rule, certs);
        let filename = format!("stream_{}_{}.conf", rule.listen_port, rule.proxy_type);
        fs::write(stream_d.join(&filename), config)?;
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
    let backup_dir = data_dir.join("nginx_backup");

    // Remove old backup if exists
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }

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
    let _ = fs::remove_dir_all(backup_dir);

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

/// Apply config generation with backup, test, and optional reload.
/// Returns conflicts detected during generation.
/// On test failure, restores the backup and returns an error.
pub fn apply_and_reload(
    data_dir: &Path,
    rules: &[ProxyRule],
    certs: &[Certificate],
    access_lists: &[(AccessList, Vec<AccessRule>)],
) -> Result<Vec<PortConflict>, AppError> {
    // Step 1: backup existing configs
    let backup_dir = backup_configs(data_dir)?;

    // Step 2: generate new configs
    let conflicts = match generate_all_configs(data_dir, rules, certs, access_lists) {
        Ok(c) => c,
        Err(e) => {
            warn!("Config generation failed, restoring backup: {}", e);
            let _ = restore_configs(&backup_dir, data_dir);
            return Err(e);
        }
    };

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
            let _ = fs::remove_dir_all(&backup_dir);
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
            // nginx binary not found or other issue - still keep new configs
            // but don't fail the operation since configs might still be valid
            warn!("Could not test config (nginx not available): {}", e);
            let _ = fs::remove_dir_all(&backup_dir);
            Ok(conflicts)
        }
    }
}
