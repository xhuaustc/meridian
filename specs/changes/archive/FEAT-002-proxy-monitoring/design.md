# FEAT-002: Proxy Monitoring — Technical Design

## Architecture Overview

```
nginx access logs (per-rule JSON files)
        │
        ▼
  Rust log parser (on-demand, not daemon)
        │
        ▼
  Aggregation engine (time-bucket grouping)
        │
        ▼
  Tauri IPC command → JSON response
        │
        ▼
  React MonitorPage (recharts)
```

**Key decision:** No persistent metrics DB. Parse log files on-demand each time the frontend requests metrics. This keeps the architecture simple — nginx logs ARE the data store. For a local proxy manager with moderate traffic, parsing a 24h log window is fast enough (< 100ms for ~100K lines on modern hardware).

## Nginx Log Format Changes

### HTTP block (`main_config.rs`)

Add custom `log_format` in the `http { }` context:

```nginx
log_format meridian escape=json
  '{"time":"$time_iso8601",'
  '"remote_addr":"$remote_addr",'
  '"method":"$request_method",'
  '"uri":"$request_uri",'
  '"status":$status,'
  '"body_bytes_sent":$body_bytes_sent,'
  '"request_time":$request_time,'
  '"upstream_response_time":"$upstream_response_time",'
  '"host":"$host"}';
```

Keep the existing global `access_log` in combined format (for LogsPage compatibility). Add per-rule log in each `location` block:

```nginx
location /api {
    access_log "{dir}/nginx/logs/rule_{rule_id}.access.log" meridian;
    proxy_pass ...;
}
```

### Stream block (`main_config.rs`)

Add `log_format` in the `stream { }` context:

```nginx
log_format stream_meridian
  '{"time":"$time_iso8601",'
  '"remote_addr":"$remote_addr",'
  '"protocol":"$protocol",'
  '"status":$status,'
  '"bytes_sent":$bytes_sent,'
  '"bytes_received":$bytes_received,'
  '"session_time":"$session_time"}';
```

Per-rule log in each stream `server` block:

```nginx
server {
    access_log "{dir}/nginx/logs/rule_{rule_id}.access.log" stream_meridian;
    listen ...;
}
```

## Log File Convention

- Path: `{data_dir}/nginx/logs/rule_{rule_id}.access.log`
- One file per enabled proxy rule
- JSON lines format (one JSON object per line)
- Rotated by clearing when exceeding 50MB (or user clears)

## Backend: Log Parser & Aggregator

### New module: `src-tauri/src/metrics/`

- `mod.rs` — public interface
- `parser.rs` — JSON line parser for both HTTP and stream formats
- `aggregator.rs` — time-bucket aggregation logic

### Parsed log entry (internal)

```rust
struct HttpLogEntry {
    time: DateTime<Utc>,
    status: u16,
    body_bytes_sent: u64,
    request_time: f64,        // seconds
    upstream_response_time: Option<f64>,
}

struct StreamLogEntry {
    time: DateTime<Utc>,
    status: u16,              // nginx stream status (200=session closed normally, 502=upstream unreachable)
    bytes_sent: u64,
    bytes_received: u64,
    session_time: f64,        // seconds
}
```

### Aggregation

Time bucket granularity by range:
| Range | Bucket size | Max points |
|-------|------------|------------|
| 1h    | 1 min      | 60         |
| 6h    | 5 min      | 72         |
| 24h   | 15 min     | 96         |

### Tauri command

```rust
#[tauri::command]
pub async fn get_proxy_metrics(
    rule_id: Option<String>,   // None = aggregate all rules
    time_range: String,        // "1h" | "6h" | "24h"
    state: State<'_, AppState>,
) -> Result<ProxyMetrics, AppError>
```

### Response shape

```rust
struct ProxyMetrics {
    summary: MetricSummary,
    time_series: Vec<MetricBucket>,
    status_distribution: Vec<StatusGroup>,
}

struct MetricSummary {
    total_requests: u64,
    error_count: u64,
    error_rate: f64,           // 0.0 – 1.0
    avg_latency_ms: f64,
    total_bytes: u64,
}

struct MetricBucket {
    timestamp: String,         // ISO 8601
    requests: u64,
    errors: u64,
    avg_latency_ms: f64,
    bytes: u64,
}

struct StatusGroup {
    group: String,             // "2xx", "3xx", "4xx", "5xx"
    count: u64,
}
```

## Frontend: MonitorPage

### Route & Navigation

- Route: `/monitor`
- Sidebar: "代理管理" group, after "仪表盘", icon: `Activity` (lucide-react)
- i18n keys: `nav.monitor` = "监控" / "Monitor"

### Page Layout

```
┌─────────────────────────────────────────────────┐
│ 监控   [Rule dropdown ▼]  [1h] [6h] [24h]  🔄  │
├─────────────────────────────────────────────────┤
│ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│ │ Requests│ │Error Rate│ │Avg Ltncy│ │Bandwdth│ │
│ │  12,345 │ │  2.3%   │ │  45ms   │ │ 1.2GB  │ │
│ └─────────┘ └─────────┘ └─────────┘ └────────┘ │
├─────────────────────────────────────────────────┤
│         Request Volume (Area Chart)              │
│  ▁▂▃▅▇█▇▅▃▂▁▁▂▃▅▇▆▅▃▂▁                        │
├───────────────────────┬─────────────────────────┤
│  Response Time (Line) │  Status Distribution    │
│  ───╱╲──╱╲───        │     ● 2xx  85%          │
│                       │     ● 4xx  12%          │
│                       │     ● 5xx   3%          │
└───────────────────────┴─────────────────────────┘
```

### Chart Library

**recharts** — React-native, declarative, lightweight (~140KB gzipped), good TypeScript support.

### Auto-refresh

Poll every 30 seconds when page is active. Stop polling when navigating away.

### Empty State

When no log data exists: show illustration + "暂无监控数据" / "No monitoring data" + hint to start the engine and generate some traffic.
