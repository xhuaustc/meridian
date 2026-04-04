use std::io::{BufRead, BufReader};

use tauri::State;

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
    state: State<'_, AppState>,
) -> Result<LogChunk, AppError> {
    let log_path = state.data_dir.join("nginx").join("logs").join("access.log");
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
        std::fs::write(&access_log, "")?;
    }
    if error_log.exists() {
        std::fs::write(&error_log, "")?;
    }

    Ok(())
}
