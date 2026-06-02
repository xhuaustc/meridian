use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{Duration, SystemTime};

use tauri::State;
use tracing::info;

use crate::error::AppError;
use crate::AppState;

#[derive(serde::Serialize)]
pub struct LogChunk {
    pub lines: Vec<String>,
    pub total_lines: usize,
}

#[tauri::command]
pub async fn read_access_log(
    tail_lines: Option<usize>,
    rule_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<LogChunk, AppError> {
    let log_path = match rule_id {
        Some(id) if !id.is_empty() => state
            .data_dir
            .join("nginx")
            .join("logs")
            .join(format!("rule_{}.access.log", id)),
        _ => state.data_dir.join("nginx").join("logs").join("access.log"),
    };
    read_log_file(&log_path, tail_lines.unwrap_or(100))
}

#[tauri::command]
pub async fn read_error_log(
    tail_lines: Option<usize>,
    state: State<'_, AppState>,
) -> Result<LogChunk, AppError> {
    let log_path = state.data_dir.join("nginx").join("logs").join("error.log");
    read_log_file(&log_path, tail_lines.unwrap_or(100))
}

fn read_log_file(path: &std::path::Path, tail: usize) -> Result<LogChunk, AppError> {
    if !path.exists() {
        return Ok(LogChunk {
            lines: Vec::new(),
            total_lines: 0,
        });
    }

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;
    let total = all_lines.len();

    let start = if total > tail { total - tail } else { 0 };
    let lines = all_lines[start..].to_vec();

    Ok(LogChunk {
        lines,
        total_lines: total,
    })
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), AppError> {
    let logs_dir = state.data_dir.join("nginx").join("logs");

    let access_log = logs_dir.join("access.log");
    let error_log = logs_dir.join("error.log");

    if access_log.exists() {
        fs::write(&access_log, "")?;
    }
    if error_log.exists() {
        fs::write(&error_log, "")?;
    }

    Ok(())
}

/// Clean up log files older than `retention_days`.
/// Truncates files whose last-modified time exceeds the retention window,
/// and removes per-rule log lines older than the cutoff by rewriting the file.
pub fn cleanup_old_logs(logs_dir: &Path, retention_days: u64) {
    if !logs_dir.exists() {
        return;
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days * 86400))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let entries = match fs::read_dir(logs_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut cleaned = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.ends_with(".log") {
            continue;
        }

        // Check file modification time
        let modified = match path.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        // If the entire file is older than cutoff, truncate it
        if modified < cutoff {
            let _ = fs::write(&path, "");
            cleaned += 1;
            continue;
        }

        // For per-rule JSON logs, filter out old lines by parsing timestamps
        if name.starts_with("rule_") {
            if let Ok(()) = trim_old_json_lines(&path, retention_days) {
                cleaned += 1;
            }
        }
    }

    if cleaned > 0 {
        info!(
            "Log cleanup: processed {} log files (retention: {} days)",
            cleaned, retention_days
        );
    }
}

/// Remove JSON log lines older than `retention_days` from a file.
fn trim_old_json_lines(path: &Path, retention_days: u64) -> Result<(), AppError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);

    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut kept = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        // Try to extract "time" field from JSON line
        if let Some(time_str) = extract_json_time(&line) {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&time_str) {
                if dt.with_timezone(&chrono::Utc) < cutoff {
                    continue; // skip old line
                }
            }
        }
        kept.push(line);
    }

    fs::write(
        path,
        kept.join("\n") + if kept.is_empty() { "" } else { "\n" },
    )?;
    Ok(())
}

/// Quick extraction of "time" value from a JSON log line without full parsing.
fn extract_json_time(line: &str) -> Option<String> {
    let marker = "\"time\":\"";
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

use crate::store::DbPool;

/// Spawn a background task that runs log cleanup every 6 hours.
pub fn spawn_log_cleanup_task(pool: DbPool, data_dir: std::path::PathBuf) {
    std::thread::spawn(move || {
        // Initial delay — startup cleanup already ran
        std::thread::sleep(std::time::Duration::from_secs(60));

        loop {
            // Sleep 6 hours
            std::thread::sleep(std::time::Duration::from_secs(6 * 60 * 60));

            let retention_days = pool
                .get()
                .ok()
                .and_then(|db| {
                    crate::store::settings_repo::get(&db, "log_retention_days")
                        .ok()
                        .flatten()
                })
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(7);

            let logs_dir = data_dir.join("nginx/logs");
            info!(
                "Running scheduled log cleanup (retention: {} days)",
                retention_days
            );
            cleanup_old_logs(&logs_dir, retention_days);
        }
    });
}
