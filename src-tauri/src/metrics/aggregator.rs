// SPEC: FEAT-002-proxy-monitoring/spec.md | TASK-004
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use super::parser::{self, LogEntry};

#[derive(Debug, Serialize, Clone)]
pub struct ProxyMetrics {
    pub summary: MetricSummary,
    pub time_series: Vec<MetricBucket>,
    pub status_distribution: Vec<StatusGroup>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MetricSummary {
    pub total_requests: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MetricBucket {
    pub timestamp: String,
    pub requests: u64,
    pub errors: u64,
    pub avg_latency_ms: f64,
    pub bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusGroup {
    pub group: String,
    pub count: u64,
}

/// Determine bucket size in seconds based on time range.
fn bucket_seconds(time_range: &str) -> i64 {
    match time_range {
        "1h" => 60,   // 1 minute buckets
        "6h" => 300,  // 5 minute buckets
        "24h" => 900, // 15 minute buckets
        _ => 60,
    }
}

/// Determine the start time based on time range.
fn range_start(time_range: &str) -> DateTime<Utc> {
    let now = Utc::now();
    match time_range {
        "1h" => now - Duration::hours(1),
        "6h" => now - Duration::hours(6),
        "24h" => now - Duration::hours(24),
        _ => now - Duration::hours(1),
    }
}

/// Truncate a timestamp to its bucket boundary.
fn bucket_key(time: &DateTime<Utc>, bucket_secs: i64) -> i64 {
    let ts = time.timestamp();
    (ts / bucket_secs) * bucket_secs
}

/// Parse all matching log entries from a single file within the time window.
fn parse_file_entries(path: &Path, start: &DateTime<Utc>) -> Vec<LogEntry> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(entry) = parser::parse_line(trimmed) {
            if &entry.time >= start {
                entries.push(entry);
            }
        }
    }

    entries
}

/// Aggregate metrics from a set of log entries.
pub fn aggregate(entries: &[LogEntry], time_range: &str) -> ProxyMetrics {
    let bsecs = bucket_seconds(time_range);
    let start = range_start(time_range);
    let now = Utc::now();

    // Summary accumulators
    let mut total_requests: u64 = 0;
    let mut error_count: u64 = 0;
    let mut total_latency: f64 = 0.0;
    let mut total_bytes: u64 = 0;
    let mut status_counts: BTreeMap<String, u64> = BTreeMap::new();

    // Time series buckets: bucket_ts -> (requests, errors, total_latency, bytes)
    let mut buckets: BTreeMap<i64, (u64, u64, f64, u64)> = BTreeMap::new();

    // Pre-fill all bucket slots
    let mut t = bucket_key(&start, bsecs);
    let end_t = bucket_key(&now, bsecs);
    while t <= end_t {
        buckets.entry(t).or_insert((0, 0, 0.0, 0));
        t += bsecs;
    }

    for entry in entries {
        total_requests += 1;
        total_bytes += entry.bytes;
        total_latency += entry.latency_ms;

        let is_error = entry.status >= 400;
        if is_error {
            error_count += 1;
        }

        // Status distribution (HTTP only)
        if entry.is_http {
            let group = match entry.status {
                200..=299 => "2xx",
                300..=399 => "3xx",
                400..=499 => "4xx",
                _ => "5xx",
            };
            *status_counts.entry(group.to_string()).or_insert(0) += 1;
        }

        // Time bucket
        let bk = bucket_key(&entry.time, bsecs);
        let slot = buckets.entry(bk).or_insert((0, 0, 0.0, 0));
        slot.0 += 1;
        if is_error {
            slot.1 += 1;
        }
        slot.2 += entry.latency_ms;
        slot.3 += entry.bytes;
    }

    let summary = MetricSummary {
        total_requests,
        error_count,
        error_rate: if total_requests > 0 {
            error_count as f64 / total_requests as f64
        } else {
            0.0
        },
        avg_latency_ms: if total_requests > 0 {
            total_latency / total_requests as f64
        } else {
            0.0
        },
        total_bytes,
    };

    let time_series: Vec<MetricBucket> = buckets
        .into_iter()
        .map(|(ts, (reqs, errs, lat, bytes))| {
            let dt = DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now());
            MetricBucket {
                timestamp: dt.to_rfc3339(),
                requests: reqs,
                errors: errs,
                avg_latency_ms: if reqs > 0 { lat / reqs as f64 } else { 0.0 },
                bytes,
            }
        })
        .collect();

    let status_distribution: Vec<StatusGroup> = ["2xx", "3xx", "4xx", "5xx"]
        .iter()
        .filter_map(|g| {
            let count = status_counts.get(*g).copied().unwrap_or(0);
            if count > 0 {
                Some(StatusGroup {
                    group: g.to_string(),
                    count,
                })
            } else {
                None
            }
        })
        .collect();

    ProxyMetrics {
        summary,
        time_series,
        status_distribution,
    }
}

/// Compute metrics for a specific rule or all rules.
pub fn compute_metrics(data_dir: &Path, rule_id: Option<&str>, time_range: &str) -> ProxyMetrics {
    let logs_dir = data_dir.join("nginx/logs");
    let start = range_start(time_range);

    let mut all_entries: Vec<LogEntry> = Vec::new();

    match rule_id {
        Some(id) => {
            let log_path = logs_dir.join(format!("rule_{}.access.log", id));
            all_entries.extend(parse_file_entries(&log_path, &start));
        }
        None => {
            // Read all rule_*.access.log files
            if let Ok(dir) = fs::read_dir(&logs_dir) {
                for entry in dir.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("rule_") && name_str.ends_with(".access.log") {
                        all_entries.extend(parse_file_entries(&entry.path(), &start));
                    }
                }
            }
        }
    }

    aggregate(&all_entries, time_range)
}
