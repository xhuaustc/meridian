// SPEC: FEAT-002-proxy-monitoring/spec.md | TASK-004
use tauri::State;

use crate::error::AppError;
use crate::metrics::aggregator::{self, ProxyMetrics};
use crate::AppState;

#[tauri::command]
pub async fn get_proxy_metrics(
    rule_id: Option<String>,
    time_range: String,
    state: State<'_, AppState>,
) -> Result<ProxyMetrics, AppError> {
    let data_dir = state.data_dir.clone();
    let metrics = aggregator::compute_metrics(&data_dir, rule_id.as_deref(), &time_range);
    Ok(metrics)
}
