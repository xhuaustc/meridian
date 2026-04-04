use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use tracing::{error, info};

use crate::error::AppError;
use crate::store::models::NginxStatus;

/// Find the nginx binary path.
pub fn get_bundled_nginx_path() -> Result<PathBuf, AppError> {
    let candidates = [
        "/opt/homebrew/bin/nginx",
        "/usr/local/bin/nginx",
        "/usr/sbin/nginx",
    ];

    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            info!("Found nginx at: {:?}", p);
            return Ok(p);
        }
    }

    // Try PATH lookup
    let output = Command::new("which")
        .arg("nginx")
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to search PATH for nginx: {}", e)))?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            let p = PathBuf::from(&path_str);
            info!("Found nginx in PATH: {:?}", p);
            return Ok(p);
        }
    }

    Err(AppError::Nginx(
        "nginx binary not found. Please install nginx.".to_string(),
    ))
}

/// Get the full path to the nginx.conf file.
fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nginx").join("nginx.conf")
}

/// Get the prefix path for nginx (used with -p flag).
fn prefix_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nginx")
}

/// Get the PID file path.
fn pid_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nginx").join("nginx.pid")
}

/// Append a message to the nginx error.log so it appears in the UI logs page.
pub fn append_to_error_log(data_dir: &Path, message: &str) {
    let log_path = data_dir.join("nginx").join("logs").join("error.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let ts = Local::now().format("%Y/%m/%d %H:%M:%S");
        let _ = writeln!(file, "{} [meridian] {}", ts, message.trim());
    }
}

/// Read the PID from the nginx pid file, if it exists and the process is running.
fn read_pid(data_dir: &Path) -> Option<u32> {
    let pid_file = pid_path(data_dir);
    if let Ok(contents) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            // Check if process is actually running
            let check = Command::new("kill").arg("-0").arg(pid.to_string()).output();
            if let Ok(output) = check {
                if output.status.success() {
                    return Some(pid);
                }
            }
        }
    }
    None
}

/// Start the nginx process.
pub fn start(data_dir: &Path) -> Result<(), AppError> {
    let nginx = get_bundled_nginx_path()?;
    let conf = config_path(data_dir);
    let prefix = prefix_path(data_dir);

    if !conf.exists() {
        return Err(AppError::Nginx(format!(
            "nginx.conf not found at {:?}. Generate config first.",
            conf
        )));
    }

    // Ensure log directory exists
    fs::create_dir_all(data_dir.join("nginx").join("logs"))?;

    // Check if already running
    if let Some(pid) = read_pid(data_dir) {
        return Err(AppError::Nginx(format!(
            "nginx is already running with PID {}",
            pid
        )));
    }

    let output = Command::new(&nginx)
        .arg("-c")
        .arg(&conf)
        .arg("-p")
        .arg(&prefix)
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to start nginx: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("nginx start failed: {}", stderr);
        append_to_error_log(data_dir, &format!("nginx start failed: {}", stderr));
        return Err(AppError::Nginx(format!("nginx start failed: {}", stderr)));
    }

    info!("nginx started successfully");
    append_to_error_log(data_dir, "nginx started successfully");
    Ok(())
}

/// Stop the nginx process gracefully.
pub fn stop(data_dir: &Path) -> Result<(), AppError> {
    let nginx = get_bundled_nginx_path()?;
    let prefix = prefix_path(data_dir);

    let conf = config_path(data_dir);

    let output = Command::new(&nginx)
        .arg("-s")
        .arg("quit")
        .arg("-c")
        .arg(&conf)
        .arg("-p")
        .arg(&prefix)
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to stop nginx: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If nginx is not running, that's fine
        if stderr.contains("no such process") || stderr.contains("is not running") {
            info!("nginx was not running");
            return Ok(());
        }
        error!("nginx stop failed: {}", stderr);
        append_to_error_log(data_dir, &format!("nginx stop failed: {}", stderr));
        return Err(AppError::Nginx(format!("nginx stop failed: {}", stderr)));
    }

    info!("nginx stopped successfully");
    append_to_error_log(data_dir, "nginx stopped successfully");
    Ok(())
}

/// Reload nginx configuration.
pub fn reload(data_dir: &Path) -> Result<(), AppError> {
    let nginx = get_bundled_nginx_path()?;
    let prefix = prefix_path(data_dir);

    let conf = config_path(data_dir);

    let output = Command::new(&nginx)
        .arg("-s")
        .arg("reload")
        .arg("-c")
        .arg(&conf)
        .arg("-p")
        .arg(&prefix)
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to reload nginx: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("nginx reload failed: {}", stderr);
        append_to_error_log(data_dir, &format!("nginx reload failed: {}", stderr));
        return Err(AppError::Nginx(format!("nginx reload failed: {}", stderr)));
    }

    info!("nginx reloaded successfully");
    append_to_error_log(data_dir, "nginx configuration reloaded");
    Ok(())
}

/// Test the nginx configuration.
pub fn test_config(data_dir: &Path) -> Result<(bool, String), AppError> {
    let nginx = get_bundled_nginx_path()?;
    let conf = config_path(data_dir);

    let output = Command::new(&nginx)
        .arg("-t")
        .arg("-c")
        .arg(&conf)
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to test nginx config: {}", e)))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    if success {
        info!("nginx config test passed");
    } else {
        error!("nginx config test failed: {}", stderr);
        append_to_error_log(data_dir, &format!("nginx config test failed: {}", stderr));
    }

    Ok((success, stderr))
}

/// Get the current nginx status.
pub fn status(data_dir: &Path) -> NginxStatus {
    let pid = read_pid(data_dir);
    let running = pid.is_some();

    let uptime_seconds = if running {
        pid.and_then(|p| get_process_uptime(p))
    } else {
        None
    };

    let error_message = if running {
        None
    } else {
        // Check if config is valid to provide error context
        match test_config(data_dir) {
            Ok((valid, msg)) => {
                if valid {
                    None
                } else {
                    Some(msg)
                }
            }
            Err(e) => Some(e.to_string()),
        }
    };

    let status = if running {
        "running".to_string()
    } else if error_message.is_some() {
        "error".to_string()
    } else {
        "stopped".to_string()
    };

    NginxStatus {
        status,
        pid,
        uptime_seconds,
        error_message,
    }
}

/// Try to get process uptime in seconds using `ps`.
fn get_process_uptime(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .arg("-o")
        .arg("etime=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let etime = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_etime(&etime)
}

/// Parse ps etime format: [[dd-]hh:]mm:ss
fn parse_etime(etime: &str) -> Option<u64> {
    let mut total_seconds: u64 = 0;
    let mut rest = etime;

    // Check for days
    if let Some((days_str, remainder)) = rest.split_once('-') {
        total_seconds += days_str.parse::<u64>().ok()? * 86400;
        rest = remainder;
    }

    let parts: Vec<&str> = rest.split(':').collect();
    match parts.len() {
        2 => {
            // mm:ss
            total_seconds += parts[0].parse::<u64>().ok()? * 60;
            total_seconds += parts[1].parse::<u64>().ok()?;
        }
        3 => {
            // hh:mm:ss
            total_seconds += parts[0].parse::<u64>().ok()? * 3600;
            total_seconds += parts[1].parse::<u64>().ok()? * 60;
            total_seconds += parts[2].parse::<u64>().ok()?;
        }
        _ => return None,
    }

    Some(total_seconds)
}
