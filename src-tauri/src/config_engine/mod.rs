pub mod conflict;
pub mod error_pages;
pub mod http_config;
pub mod main_config;
pub mod security_headers;
pub mod stream_config;

#[cfg(test)]
pub mod test_support;

use serde::Serialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
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
use crate::request_params::{self, RequestDefaults};
use crate::store::models::{
    AccessList, AccessRule, Certificate, HostnameSettings, PortConflict, ProxyRule,
};
use crate::store::{access_repo, cert_repo, hostname_settings_repo, proxy_repo, settings_repo};

pub struct ConfigData {
    pub rules: Vec<ProxyRule>,
    pub certs: Vec<Certificate>,
    pub access_lists: Vec<(AccessList, Vec<AccessRule>)>,
    pub hostname_settings: HashMap<String, HostnameSettings>,
    pub worker_processes: String,
    /// Global request-parameter defaults every rule inherits from.
    pub request_defaults: RequestDefaults,
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
            &data.hostname_settings,
            &data.worker_processes,
            &data.request_defaults,
            crate::nginx_manager::capabilities(),
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
    let hostname_settings = hostname_settings_repo::list_all(db)?;
    let worker_processes =
        settings_repo::get(db, "worker_processes")?.unwrap_or_else(|| "2".to_string());
    let request_defaults = request_params::load_defaults(db)?;
    Ok(ConfigData {
        rules,
        certs,
        access_lists,
        hostname_settings,
        worker_processes,
        request_defaults,
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
        &data.hostname_settings,
        &data.worker_processes,
        &data.request_defaults,
        crate::nginx_manager::capabilities(),
    )
}

/// Generate all configs with explicit settings.
/// `worker_processes`: "auto" or a numeric string (default "2").
///
/// `hostname_settings` carries the per-hostname security switches; hostnames
/// without a row keep the default headers.
/// `request_defaults` is what rules inherit their request parameters from.
pub fn generate_all_configs_with_settings(
    data_dir: &Path,
    rules: &[ProxyRule],
    certs: &[Certificate],
    access_lists: &[(AccessList, Vec<AccessRule>)],
    hostname_settings: &HashMap<String, HostnameSettings>,
    worker_processes: &str,
    request_defaults: &RequestDefaults,
    capabilities: crate::nginx_manager::Capabilities,
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
    let main_conf = main_config::generate_main_config(data_dir, worker_processes, capabilities);
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

    // Hostnames that real rules already serve on port 80. A synthesised
    // redirect would be a second `server_name` on that port, so it is skipped
    // for them; `check_force_https` tells the form why.
    let real_port_80_hosts: HashSet<&str> = http_rules
        .iter()
        .filter(|rule| rule.listen_port == http_config::REDIRECT_LISTEN_PORT)
        .filter_map(|rule| rule.domain.as_deref())
        .collect();

    // Pick which generated file carries each hostname's synthesised redirect:
    // the lowest HTTPS port for that hostname, so a hostname served on several
    // HTTPS ports still gets exactly one `listen 80` block.
    let mut redirect_owner: HashMap<&str, u16> = HashMap::new();
    for ((port, domain), group_rules) in &http_groups {
        if real_port_80_hosts.contains(domain.as_str())
            || !group_rules
                .iter()
                .any(|rule| http_config::wants_https_redirect(rule))
        {
            continue;
        }
        redirect_owner
            .entry(domain.as_str())
            .and_modify(|owner| *owner = (*owner).min(*port))
            .or_insert(*port);
    }

    // Generate HTTP config files
    for ((port, domain), group_rules) in &http_groups {
        let mut config = http_config::generate_server_block(
            group_rules,
            certs,
            access_lists,
            data_dir,
            hostname_settings,
            request_defaults,
        );
        if redirect_owner.get(domain.as_str()) == Some(port) {
            config.push('\n');
            config.push_str(&http_config::generate_https_redirect_block(domain));
        }
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
    hostname_settings: &HashMap<String, HostnameSettings>,
    worker_processes: &str,
    request_defaults: &RequestDefaults,
    capabilities: crate::nginx_manager::Capabilities,
) -> Result<Vec<PortConflict>, AppError> {
    // Step 1: backup existing configs
    let backup_dir = backup_configs(data_dir)?;

    // Step 2: generate new configs
    let conflicts = match generate_all_configs_with_settings(
        data_dir,
        rules,
        certs,
        access_lists,
        hostname_settings,
        worker_processes,
        request_defaults,
        capabilities,
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
        // Pin the capabilities instead of letting the process-wide probe cache
        // latch onto this host's module set for the rest of the test binary.
        let preview = crate::nginx_manager::with_capabilities(
            crate::nginx_manager::Capabilities::default(),
            || preview_db_state(&db, &data_dir).unwrap(),
        );
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

    fn http_rule(id: &str, port: u16, domain: &str) -> ProxyRule {
        ProxyRule {
            id: id.to_string(),
            name: id.to_string(),
            proxy_type: "http".to_string(),
            enabled: true,
            listen_port: port,
            listen_host: "0.0.0.0".to_string(),
            domain: Some(domain.to_string()),
            path_prefix: None,
            upstream_host: "127.0.0.1".to_string(),
            upstream_port: 3000,
            upstream_scheme: "http".to_string(),
            tls_mode: "none".to_string(),
            certificate_id: None,
            access_list_id: None,
            websocket: false,
            keep_alive: false,
            force_https: false,
            custom_headers: None,
            upstream_targets: None,
            request: Default::default(),
            sort_order: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn https_rule(id: &str, port: u16, domain: &str, force_https: bool) -> ProxyRule {
        let mut rule = http_rule(id, port, domain);
        rule.tls_mode = "terminate".to_string();
        rule.force_https = force_https;
        rule
    }

    fn temp_data_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("meridian-{}-{}", label, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn generate_into(
        data_dir: &Path,
        rules: &[ProxyRule],
        certs: &[Certificate],
    ) -> Vec<PortConflict> {
        generate_into_with_settings(data_dir, rules, certs, &HashMap::new())
    }

    fn generate_into_with_settings(
        data_dir: &Path,
        rules: &[ProxyRule],
        certs: &[Certificate],
        hostname_settings: &HashMap<String, HostnameSettings>,
    ) -> Vec<PortConflict> {
        generate_all_configs_with_settings(
            data_dir,
            rules,
            certs,
            &[],
            hostname_settings,
            "1",
            &RequestDefaults::builtin(),
            crate::nginx_manager::Capabilities::default(),
        )
        .unwrap()
    }

    /// Every generated conf.d file that carries the synthesised redirect, with
    /// the number of blocks it holds.
    fn redirect_blocks_in(data_dir: &Path) -> Vec<(String, usize)> {
        let mut found = Vec::new();
        for entry in fs::read_dir(data_dir.join("nginx/conf.d"))
            .unwrap()
            .flatten()
        {
            let content = fs::read_to_string(entry.path()).unwrap();
            let count = content
                .matches("return 308 https://$host$request_uri;")
                .count();
            if count > 0 {
                found.push((entry.file_name().to_string_lossy().to_string(), count));
            }
        }
        found.sort();
        found
    }

    fn conf_files_in(data_dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(data_dir.join("nginx/conf.d"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".conf"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn force_https_rule_synthesises_a_redirect_in_its_own_file() {
        let data_dir = temp_data_dir("redirect");
        let rules = vec![https_rule("tls", 443, "app.local", true)];
        let conflicts = generate_into(&data_dir, &rules, &[]);

        assert!(conflicts.is_empty(), "the redirect must not add conflicts");
        let config = fs::read_to_string(data_dir.join("nginx/conf.d/443_app_local.conf")).unwrap();
        assert!(config.contains("    listen 443 ssl;"));
        assert!(config.contains(concat!(
            "    listen 80;\n",
            "    server_name app.local;\n",
            "    return 308 https://$host$request_uri;\n",
        )));
        assert_eq!(
            redirect_blocks_in(&data_dir),
            vec![("443_app_local.conf".to_string(), 1)],
            "the redirect belongs to the hostname's own file"
        );
        assert_eq!(
            conf_files_in(&data_dir),
            vec!["443_app_local.conf".to_string()]
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn force_https_skips_a_hostname_that_already_has_a_port_80_rule() {
        let data_dir = temp_data_dir("redirect-skip");
        let rules = vec![
            http_rule("plain", 80, "app.local"),
            https_rule("tls", 443, "app.local", true),
        ];
        assert!(generate_into(&data_dir, &rules, &[]).is_empty());

        assert!(
            redirect_blocks_in(&data_dir).is_empty(),
            "a real port-80 server block must not be shadowed by a synthesised one"
        );
        let plain = fs::read_to_string(data_dir.join("nginx/conf.d/80_app_local.conf")).unwrap();
        assert!(plain.contains("    listen 80;\n    server_name app.local;\n"));
        assert!(plain.contains("proxy_pass"));
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn turning_force_https_off_leaves_no_redirect_residue() {
        let data_dir = temp_data_dir("redirect-off");
        generate_into(&data_dir, &[https_rule("tls", 443, "app.local", true)], &[]);
        assert_eq!(redirect_blocks_in(&data_dir).len(), 1);

        generate_into(
            &data_dir,
            &[https_rule("tls", 443, "app.local", false)],
            &[],
        );
        assert!(redirect_blocks_in(&data_dir).is_empty());
        let config = fs::read_to_string(data_dir.join("nginx/conf.d/443_app_local.conf")).unwrap();
        assert!(!config.contains("listen 80;"));
        assert_eq!(
            conf_files_in(&data_dir),
            vec!["443_app_local.conf".to_string()]
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn one_redirect_block_per_hostname_even_with_several_https_ports() {
        let data_dir = temp_data_dir("redirect-ports");
        let rules = vec![
            https_rule("tls-8443", 8443, "app.local", true),
            https_rule("tls-443", 443, "app.local", true),
        ];
        assert!(generate_into(&data_dir, &rules, &[]).is_empty());

        assert_eq!(
            redirect_blocks_in(&data_dir),
            vec![("443_app_local.conf".to_string(), 1)],
            "the lowest HTTPS port owns the redirect, so `listen 80` appears once"
        );
        assert_eq!(
            conf_files_in(&data_dir),
            vec![
                "443_app_local.conf".to_string(),
                "8443_app_local.conf".to_string()
            ]
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn the_redirect_block_adds_no_rule_no_file_and_no_conflict() {
        let data_dir = temp_data_dir("redirect-count");
        let rules = vec![
            https_rule("tls", 443, "app.local", true),
            http_rule("plain", 80, "other.local"),
        ];
        let conflicts = generate_into(&data_dir, &rules, &[]);

        assert!(conflicts.is_empty());
        // Two rules in two groups, two files: the redirect gets none of its own.
        assert_eq!(
            conf_files_in(&data_dir),
            vec![
                "443_app_local.conf".to_string(),
                "80_other_local.conf".to_string()
            ]
        );
        assert_eq!(
            redirect_blocks_in(&data_dir),
            vec![("443_app_local.conf".to_string(), 1)]
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn load_config_data_reads_hostname_settings_from_the_database() {
        let data_dir = temp_data_dir("load-hostname-settings");
        let db = crate::store::init_database(&data_dir.join("meridian.db")).unwrap();
        db.execute(
            "INSERT INTO hostname_settings
                 (hostname, sec_nosniff, created_at, updated_at)
             VALUES ('app.local', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let data = load_config_data(&db).unwrap();
        let settings = data
            .hostname_settings
            .get("app.local")
            .expect("the migration-created table row must reach generation");
        assert!(!settings.sec_nosniff);
        assert!(settings.sec_referrer_policy);
        assert!(settings.sec_frame_options);

        drop(db);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn a_hostname_settings_row_turns_off_one_header_in_the_generated_file() {
        let data_dir = temp_data_dir("hostname-settings");
        let settings = HashMap::from([(
            "app.local".to_string(),
            HostnameSettings {
                hostname: "app.local".to_string(),
                sec_nosniff: false,
                ..HostnameSettings::default()
            },
        )]);
        assert!(generate_into_with_settings(
            &data_dir,
            &[http_rule("plain", 80, "app.local")],
            &[],
            &settings,
        )
        .is_empty());

        let config = fs::read_to_string(data_dir.join("nginx/conf.d/80_app_local.conf")).unwrap();
        assert!(
            !config.contains("X-Content-Type-Options"),
            "the hostname's switch must remove the header from its server block"
        );
        assert!(
            config.contains("add_header Referrer-Policy strict-origin-when-cross-origin always;")
        );
        assert!(config.contains("add_header X-Frame-Options SAMEORIGIN always;"));
        let _ = fs::remove_dir_all(&data_dir);
    }

    /// The synthesised block must be accepted by the nginx binary we ship, not
    /// just by our own string checks. Skips (loudly) when the sidecar has not
    /// been built on this machine.
    #[test]
    fn synthesised_redirect_block_is_accepted_by_the_bundled_nginx() {
        let Some(nginx) = crate::nginx_manager::repo_sidecar() else {
            eprintln!(
                "skipping: run scripts/prepare-nginx.sh to validate the synthesised redirect block against the sidecar"
            );
            return;
        };

        let data_dir = temp_data_dir("redirect-nginx-t");
        let generated =
            crate::cert_manager::generate_self_signed(&data_dir, "test", "app.local", 2).unwrap();
        let cert = Certificate {
            id: "cert-1".to_string(),
            name: generated.name,
            domain: generated.domain,
            cert_path: generated.cert_path,
            key_path: generated.key_path,
            source: generated.source,
            expires_at: generated.expires_at,
            auto_renew: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            dns_credential_id: None,
            acme_account_id: None,
            acme_domains: None,
            last_renew_error: None,
            last_renew_at: None,
            status: "ready".to_string(),
        };
        let mut rule = https_rule("tls", 443, "app.local", true);
        rule.certificate_id = Some(cert.id.clone());
        assert!(generate_into(&data_dir, &[rule], &[cert]).is_empty());

        let nginx_dir = data_dir.join("nginx");
        for dir in [
            nginx_dir.join("temp/client_body"),
            nginx_dir.join("temp/proxy"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        // nginx refuses an include whose glob matches nothing.
        fs::write(nginx_dir.join("stream.d/.keep.conf"), "# placeholder\n").unwrap();

        let config_path = nginx_dir.join("nginx.conf");
        let output = std::process::Command::new(&nginx)
            .arg("-t")
            .arg("-c")
            .arg(&config_path)
            .arg("-p")
            .arg(&nginx_dir)
            .output()
            .expect("failed to run nginx -t");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "nginx -t rejected the config with the synthesised redirect block:\n{}",
            stderr
        );
        assert!(
            stderr.contains("test is successful"),
            "unexpected nginx -t output: {}",
            stderr
        );

        let config = fs::read_to_string(nginx_dir.join("conf.d/443_app_local.conf")).unwrap();
        assert!(
            config.contains("return 308 https://$host$request_uri;"),
            "the validated config must be the one with the redirect block"
        );
        let _ = fs::remove_dir_all(&data_dir);
    }
}

/// Example usage of the shared harness. A later config ticket adds rule shapes
/// here, calls the production generator, and ends with the repository sidecar
/// judging the result instead of hand-rolling its own `nginx -t` invocation.
#[cfg(test)]
mod sidecar_tests {
    use std::collections::HashMap;

    use super::test_support::{
        self, assert_every_add_header_applies_to_error_responses,
        assert_no_add_header_in_locations, count_directive_in_blocks, SidecarHarness,
    };

    #[test]
    fn generated_http_and_stream_configs_are_accepted_by_the_sidecar() {
        let Some((http_config, stream_config)) = SidecarHarness::with_configs(|harness| {
            let rules = [
                test_support::http_rule("rule-http-000000000001", "app.local"),
                test_support::stream_rule("rule-stream-0000000001", 15432),
            ];
            harness.generate_all(
                &rules,
                &[],
                &[],
                &HashMap::new(),
                "1",
                harness.capabilities(),
            );
            (
                harness.read("conf.d/18080_app_local.conf"),
                harness.read("stream.d/stream_15432_stream_tcp.conf"),
            )
        }) else {
            return;
        };

        assert_eq!(
            count_directive_in_blocks(&http_config, "server", "listen"),
            1,
            "the generated server block must listen exactly once"
        );
        assert_eq!(
            count_directive_in_blocks(&http_config, "location /", "proxy_set_header"),
            4,
            "the plain HTTP location must keep forwarding the four proxy headers"
        );
        assert_eq!(
            count_directive_in_blocks(&stream_config, "server", "proxy_pass"),
            1
        );
    }

    #[test]
    fn the_default_security_headers_are_accepted_by_the_sidecar() {
        let Some((http_config, stream_config)) = SidecarHarness::with_configs(|harness| {
            let rules = [
                test_support::http_rule("rule-http-000000000002", "headers.local"),
                test_support::stream_rule("rule-stream-0000000002", 15433),
            ];
            harness.generate_all(
                &rules,
                &[],
                &[],
                &HashMap::new(),
                "1",
                harness.capabilities(),
            );
            (
                harness.read("conf.d/18080_headers_local.conf"),
                harness.read("stream.d/stream_15433_stream_tcp.conf"),
            )
        }) else {
            return;
        };

        assert_eq!(
            count_directive_in_blocks(&http_config, "server", "add_header"),
            3,
            "every HTTP server block carries the complete default header list"
        );
        assert_eq!(
            count_directive_in_blocks(&stream_config, "server", "add_header"),
            0,
            "response headers belong to the HTTP layer; stream configs never carry them"
        );
        assert_no_add_header_in_locations(&http_config);
        assert_every_add_header_applies_to_error_responses(&http_config);
    }

    #[test]
    fn custom_request_parameters_are_accepted_by_the_sidecar() {
        let Some(()) = SidecarHarness::with_configs(|harness| {
            let mut rule = test_support::http_rule("rule-http-000000000002", "app.local");
            rule.request.client_max_body_size = Some("2g".to_string());
            rule.request.client_body_timeout = Some("300s".to_string());
            rule.request.proxy_connect_timeout = Some("10s".to_string());
            rule.request.proxy_read_timeout = Some("3600s".to_string());
            rule.request.proxy_send_timeout = Some("3600s".to_string());
            rule.request.proxy_buffering = Some("off".to_string());

            harness.generate_all(
                &[rule],
                &[],
                &[],
                &HashMap::new(),
                "1",
                harness.capabilities(),
            );

            let config = harness.read("conf.d/18080_app_local.conf");
            assert_eq!(
                count_directive_in_blocks(&config, "location /", "client_max_body_size"),
                1,
                "the overridden body limit must reach the location nginx reads"
            );
            assert!(config.contains("        client_max_body_size 2g;"));
            assert!(config.contains("        client_body_timeout 300s;"));
            assert!(config.contains("        proxy_connect_timeout 10s;"));
            assert!(config.contains("        proxy_read_timeout 3600s;"));
            assert!(config.contains("        proxy_send_timeout 3600s;"));
            assert!(config.contains("        proxy_buffering off;"));
        }) else {
            return;
        };
    }
}
