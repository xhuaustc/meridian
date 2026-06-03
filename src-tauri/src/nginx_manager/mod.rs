use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use tauri::Emitter;
use tracing::{error, info, warn};

use crate::error::AppError;
use crate::store::models::NginxStatus;

/// Create a Command for the nginx binary with platform-specific settings.
/// On Windows, sets CREATE_NO_WINDOW to prevent console window flashing.
fn nginx_command(nginx_path: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(nginx_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

static WAS_RUNNING: AtomicBool = AtomicBool::new(false);

/// Spawn a background health check that emits "nginx-status-changed" event when nginx crashes.
pub fn spawn_health_check(data_dir: PathBuf, app_handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(15));
        let current_status = status(&data_dir);
        let is_running = current_status.status == "running";
        let was_running = WAS_RUNNING.swap(is_running, Ordering::Relaxed);

        if was_running && !is_running {
            warn!("nginx stopped unexpectedly");
            append_to_error_log(&data_dir, "nginx process stopped unexpectedly");
            let _ = app_handle.emit(
                "nginx-status-changed",
                serde_json::json!({
                    "status": current_status.status,
                    "error_message": current_status.error_message,
                }),
            );
        }
    });
}

/// The nginx binary file name on the current platform.
#[cfg(windows)]
const NGINX_BIN_NAME: &str = "nginx.exe";
#[cfg(not(windows))]
const NGINX_BIN_NAME: &str = "nginx";

/// Find the nginx binary path.
///
/// Resolution order:
/// 1. Bundled sidecar — binary next to the app executable (production builds)
/// 2. Well-known system paths (development fallback, Unix only)
/// 3. PATH lookup (development fallback)
pub fn get_bundled_nginx_path() -> Result<PathBuf, AppError> {
    // 1. Bundled sidecar: look next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = dir.join(NGINX_BIN_NAME);
            if sidecar.exists() {
                info!("Found bundled nginx at: {:?}", sidecar);
                return Ok(sidecar);
            }
        }
    }

    // 2. Well-known system paths (Unix dev mode)
    #[cfg(not(windows))]
    {
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
    }

    // 3. PATH lookup
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let mut cmd = Command::new(which_cmd);
    cmd.arg("nginx");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| AppError::Nginx(format!("Failed to search PATH for nginx: {}", e)))?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !path_str.is_empty() {
            let p = PathBuf::from(&path_str);
            info!("Found nginx in PATH: {:?}", p);
            return Ok(p);
        }
    }

    Err(AppError::Nginx(
        "nginx binary not found. Expected bundled sidecar or system nginx.".to_string(),
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

/// Clean up a stale nginx process on startup.
///
/// If a PID file exists but the process is no longer running, the stale file is
/// removed.  If the process *is* still running (left over from a previous
/// session), we attempt a graceful shutdown (`nginx -s quit`) and, after a
/// timeout, force-kill the process.
pub fn cleanup_stale_process(data_dir: &Path) {
    let pid_file = pid_path(data_dir);
    let contents = match fs::read_to_string(&pid_file) {
        Ok(c) => c,
        Err(_) => return, // No PID file — nothing to clean up.
    };

    let pid: u32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            info!("Removing unparseable PID file");
            append_to_error_log(data_dir, "Removing unparseable nginx PID file on startup");
            let _ = fs::remove_file(&pid_file);
            return;
        }
    };

    if !is_process_running(pid) {
        info!("Removing stale nginx PID file (pid {} not running)", pid);
        append_to_error_log(
            data_dir,
            &format!(
                "Removed stale nginx PID file on startup (pid {} not running)",
                pid
            ),
        );
        let _ = fs::remove_file(&pid_file);
        return;
    }

    // Process is still alive — attempt graceful stop.
    info!(
        "Found running nginx process (pid {}) from previous session, stopping it",
        pid
    );
    append_to_error_log(
        data_dir,
        &format!(
            "Found running nginx process (pid {}) from previous session, attempting graceful stop",
            pid
        ),
    );

    if let Ok(nginx) = get_bundled_nginx_path() {
        let _ = nginx_command(&nginx)
            .arg("-s")
            .arg("quit")
            .arg("-c")
            .arg(config_path(data_dir))
            .arg("-p")
            .arg(prefix_path(data_dir))
            .output();
    }

    // Wait up to 3 seconds for the process to exit.
    let mut stopped = false;
    for _ in 0..6 {
        thread::sleep(Duration::from_millis(500));
        if !is_process_running(pid) {
            stopped = true;
            break;
        }
    }

    if stopped {
        info!("Stale nginx process (pid {}) stopped gracefully", pid);
        append_to_error_log(
            data_dir,
            &format!(
                "Stale nginx process (pid {}) stopped gracefully on startup",
                pid
            ),
        );
    } else {
        // Force kill.
        info!(
            "Stale nginx process (pid {}) did not stop gracefully, force killing",
            pid
        );
        append_to_error_log(
            data_dir,
            &format!("Force killing stale nginx process (pid {}) on startup", pid),
        );

        #[cfg(not(windows))]
        {
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }

    // Clean up PID file if it still exists.
    let _ = fs::remove_file(&pid_file);
}

/// Read the PID from the nginx pid file, if it exists and the process is running.
fn read_pid(data_dir: &Path) -> Option<u32> {
    let pid_file = pid_path(data_dir);
    let contents = fs::read_to_string(&pid_file).ok()?;
    let pid = contents.trim().parse::<u32>().ok()?;
    if is_process_running(pid) {
        Some(pid)
    } else {
        None
    }
}

/// Check whether a process with the given PID is alive.
#[cfg(not(windows))]
fn is_process_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).contains(&pid.to_string())
        })
        .unwrap_or(false)
}

fn wait_until_process_exits<F>(timeout: Duration, interval: Duration, mut is_running: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !is_running() {
            return true;
        }
        thread::sleep(interval);
    }
    !is_running()
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

    let output = nginx_command(&nginx)
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
    let pid = read_pid(data_dir);

    let output = nginx_command(&nginx)
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
        if stderr.contains("no such process")
            || stderr.contains("is not running")
            || stderr.contains("error") && read_pid(data_dir).is_none()
        {
            info!("nginx was not running");
            let _ = fs::remove_file(pid_path(data_dir));
            WAS_RUNNING.store(false, Ordering::Relaxed);
            return Ok(());
        }
        error!("nginx stop failed: {}", stderr);
        append_to_error_log(data_dir, &format!("nginx stop failed: {}", stderr));
        return Err(AppError::Nginx(format!("nginx stop failed: {}", stderr)));
    }

    if let Some(pid) = pid {
        let stopped =
            wait_until_process_exits(Duration::from_secs(3), Duration::from_millis(100), || {
                is_process_running(pid)
            });
        if !stopped {
            let msg = format!("nginx did not stop within timeout (pid {})", pid);
            error!("{}", msg);
            append_to_error_log(data_dir, &msg);
            return Err(AppError::Nginx(msg));
        }
    }

    let _ = fs::remove_file(pid_path(data_dir));
    WAS_RUNNING.store(false, Ordering::Relaxed);
    info!("nginx stopped successfully");
    append_to_error_log(data_dir, "nginx stopped successfully");
    Ok(())
}

/// Reload nginx configuration.
pub fn reload(data_dir: &Path) -> Result<(), AppError> {
    let nginx = get_bundled_nginx_path()?;
    let prefix = prefix_path(data_dir);

    let conf = config_path(data_dir);

    let output = nginx_command(&nginx)
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

    let output = nginx_command(&nginx)
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
    } else if !config_path(data_dir).exists() {
        // Config not generated yet (first launch) — don't spawn nginx to test
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

/// Try to get process uptime in seconds.
#[cfg(not(windows))]
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

#[cfg(windows)]
fn get_process_uptime(pid: u32) -> Option<u64> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    // Use PowerShell Get-Process instead of deprecated wmic
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-Process -Id {}).StartTime.ToString('yyyyMMddHHmmss')",
                pid
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.len() >= 14 {
        let year: i32 = stdout[0..4].parse().ok()?;
        let month: u32 = stdout[4..6].parse().ok()?;
        let day: u32 = stdout[6..8].parse().ok()?;
        let hour: u32 = stdout[8..10].parse().ok()?;
        let min: u32 = stdout[10..12].parse().ok()?;
        let sec: u32 = stdout[12..14].parse().ok()?;

        let created =
            chrono::NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, min, sec)?;
        let now = chrono::Local::now().naive_local();
        let duration = now.signed_duration_since(created);
        return Some(duration.num_seconds().max(0) as u64);
    }
    None
}

/// Parse ps etime format: [[dd-]hh:]mm:ss
#[cfg(not(windows))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn wait_until_process_exits_stops_polling_after_process_disappears() {
        let mut checks = 0;

        let stopped =
            wait_until_process_exits(Duration::from_millis(50), Duration::from_millis(1), || {
                checks += 1;
                checks < 3
            });

        assert!(stopped);
        assert_eq!(checks, 3);
    }

    #[test]
    fn wait_until_process_exits_returns_false_after_timeout() {
        let stopped =
            wait_until_process_exits(Duration::from_millis(3), Duration::from_millis(1), || true);

        assert!(!stopped);
    }
}
