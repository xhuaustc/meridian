# Spec: Proxy Monitoring (代理监控)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec (merged from FEAT-002) | Feature merge |

## Feature Description

为代理规则提供可视化监控面板，展示请求数、错误率、响应时间、流量吞吐等核心指标，支持按规则过滤和时间范围选择，包含 HTTP 和 TCP/UDP 代理。数据源为 nginx 日志文件，按需解析聚合（无持久化指标数据库）。

## Use Cases

- UC-001: 查看全局代理监控概览（所有规则聚合）
- UC-002: 按单条代理规则过滤查看其独立指标
- UC-003: 切换时间范围（1h / 6h / 24h）查看不同粒度
- UC-004: 查看请求量时间趋势图（Area Chart）
- UC-005: 查看响应时间趋势图（Line Chart）
- UC-006: 查看 HTTP 状态码分布（Pie Chart）
- UC-007: 查看 TCP/UDP 连接和流量指标

## Interface Definition

### Tauri Command: `get_proxy_metrics`

**Input:**
```typescript
{
  ruleId?: string;    // undefined = all rules
  timeRange: string;  // "1h" | "6h" | "24h"
}
```

**Output:**
```typescript
interface ProxyMetrics {
  summary: {
    total_requests: number;
    error_count: number;
    error_rate: number;       // 0.0 – 1.0
    avg_latency_ms: number;
    total_bytes: number;
  };
  time_series: Array<{
    timestamp: string;        // ISO 8601
    requests: number;
    errors: number;
    avg_latency_ms: number;
    bytes: number;
  }>;
  status_distribution: Array<{
    group: string;            // "2xx" | "3xx" | "4xx" | "5xx"
    count: number;
  }>;
}
```

### Nginx Log Formats

#### HTTP (`meridian` log_format)
JSON lines: `time`, `remote_addr`, `method`, `uri`, `status`, `body_bytes_sent`, `request_time`, `upstream_response_time`, `host`

#### Stream (`stream_meridian` log_format)
JSON lines: `time`, `remote_addr`, `protocol`, `status`, `bytes_sent`, `bytes_received`, `session_time`

### Per-Rule Log Files

Path: `{data_dir}/nginx/logs/rule_{rule_id}.access.log`

- HTTP: `access_log` in each `location` block (meridian format)
- Stream: `access_log` in each `server` block (stream_meridian format)

### Backend: Log Parser

- `parse_line(line)` → `LogEntry` (tries HTTP format first, falls back to stream)
- Handles `upstream_response_time` being "-" or comma-separated values

### Backend: Metrics Aggregator

- `compute_metrics(data_dir, rule_id, time_range)` → `ProxyMetrics`
- Time bucketing: 1h → 60 buckets (1min), 6h → 72 (5min), 24h → 96 (15min)
- Pre-fills all bucket slots for continuous chart display
- Reads all `rule_*.access.log` files (all rules) or specific one

### Frontend: MonitorPage

- Route: `/monitor`, sidebar: Activity icon under 代理管理 group
- Rule filter: custom Select dropdown (w-44)
- Time range: pill buttons (1h / 6h / 24h)
- Stat cards (4): Total Requests, Error Rate, Avg Latency, Bandwidth
- Charts: recharts AreaChart, LineChart, PieChart with CSS variable theming
- Auto-refresh: 30 seconds
- Empty state when no data

## Data Model

No new database tables. Data source is nginx log files parsed on-demand.

## Business Rules

1. **BR-001**: Per-rule log files created as rules are enabled. Stale logs acceptable; cleaned on config generation.
2. **BR-002**: Time range filtering by timestamp comparison during parsing.
3. **BR-003**: Missing/empty log file → zero metrics, no error.
4. **BR-004**: "All rules" view aggregates across all rule log files in logs directory.
5. **BR-005**: HTTP error = status >= 400. Stream error = status >= 400.
6. **BR-006**: `avg_latency_ms` — HTTP uses `request_time`, stream uses `session_time`.
7. **BR-007**: `total_bytes` — HTTP = `body_bytes_sent`, stream = `bytes_sent + bytes_received`.
8. **BR-008**: Status distribution only for HTTP rules (TCP/UDP excluded from pie chart).
9. **BR-009**: Global `access_log` in combined format kept for LogsPage backward compatibility.
10. **BR-010**: Auto-refresh interval 30 seconds.

## Edge Cases

- **EC-001**: Rule has no traffic → all metrics 0, charts show flat zero line
- **EC-002**: Malformed JSON log line → skip line, no error
- **EC-003**: Log file > 50MB → parse only lines within time window
- **EC-004**: nginx not running → show last known data from log files
- **EC-005**: Rule deleted but log file remains → "all rules" includes orphaned data; per-rule view won't show (not in dropdown)
- **EC-006**: `upstream_response_time` is "-" → treat as 0ms
- **EC-007**: No rules exist → empty state message

## Test Points

| TP-ID | Category | Input | Expected Output |
|-------|----------|-------|-----------------|
| TP-001 | Normal | Metrics for all rules, 1h range | Summary + 60 time buckets + status distribution |
| TP-002 | Normal | Metrics for specific HTTP rule | Only that rule's data |
| TP-003 | Normal | Metrics for specific TCP rule | Summary with bytes, no status distribution |
| TP-004 | Normal | Switch time range 1h → 24h | Bucket granularity 1min → 15min |
| TP-005 | Normal | Page auto-refreshes after 30s | New data without manual action |
| TP-006 | Boundary | Rule with zero traffic | All metrics 0 |
| TP-007 | Boundary | Malformed JSON log line | Line skipped, valid lines parsed |
| TP-008 | Boundary | upstream_response_time is "-" | Treated as 0ms |
| TP-009 | Error | Log file does not exist | Zero metrics, no error |
| TP-010 | Normal | Navigate to /monitor | Page renders with filters and charts |
| TP-011 | Normal | Sidebar shows "监控"/"Monitor" | Correct label under 代理管理 |
| TP-012 | Normal | HTTP rule traffic → check log file | rule_{id}.access.log with JSON lines |
| TP-013 | Normal | Stream rule traffic → check log file | rule_{id}.access.log with stream JSON |
| TP-014 | Combination | Multiple rules, filter by one, then all | Correctly scoped then aggregated |
| TP-015 | Normal | All three charts render | Area, line, pie charts visible with data |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Log parser | `src-tauri/src/metrics/parser.rs` | `parse_line()`, `HttpLogEntry`, `StreamLogEntry` | |
| Metrics aggregator | `src-tauri/src/metrics/aggregator.rs` | `compute_metrics()`, `ProxyMetrics` | |
| Metrics command | `src-tauri/src/commands/metrics.rs` | `get_proxy_metrics` | Tauri IPC |
| Log formats | `src-tauri/src/config_engine/main_config.rs` | `generate_main_config()` | meridian + stream_meridian |
| Per-rule access_log | `src-tauri/src/config_engine/http_config.rs`, `stream_config.rs` | `generate_server_block()`, `generate_stream_block()` | |
| Frontend page | `src/pages/MonitorPage.tsx` | `MonitorPage` | recharts |
| Frontend types | `src/types/index.ts` | `ProxyMetrics`, `MetricSummary`, etc. | |
| Frontend API | `src/lib/api.ts` | `getProxyMetrics()` | |
| Sidebar entry | `src/components/layout/Sidebar.tsx` | — | Activity icon |
| i18n keys | `src/locales/{zh,en}/common.json` | `monitor.*` | 18 keys |
