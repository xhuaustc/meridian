# FEAT-002: Proxy Monitoring — Feature Spec

## Feature Description

为代理规则提供可视化监控面板，展示请求数、错误率、响应时间、流量吞吐等核心指标，支持按规则过滤和时间范围选择，包含 HTTP 和 TCP/UDP 代理。

## Use Cases

- UC-001: 查看全局代理监控概览（所有规则聚合）
- UC-002: 按单条代理规则过滤查看其独立指标
- UC-003: 切换时间范围（1h / 6h / 24h）查看不同粒度
- UC-004: 查看请求量时间趋势图
- UC-005: 查看响应时间趋势图
- UC-006: 查看 HTTP 状态码分布
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

### Nginx Log Format

#### HTTP (`meridian` log_format)
JSON lines with fields: `time`, `remote_addr`, `method`, `uri`, `status`, `body_bytes_sent`, `request_time`, `upstream_response_time`, `host`

#### Stream (`stream_meridian` log_format)
JSON lines with fields: `time`, `remote_addr`, `protocol`, `status`, `bytes_sent`, `bytes_received`, `session_time`

### Per-Rule Log Files

Path convention: `{data_dir}/nginx/logs/rule_{rule_id}.access.log`

- HTTP rules: `access_log` directive inside each `location` block
- Stream rules: `access_log` directive inside each `server` block

## Data Model

No new database tables. Data source is nginx log files parsed on-demand.

Existing models impacted:
- **ProxyRule**: no schema change. `rule.id` used to locate per-rule log file.

## Business Rules

1. **BR-001**: Per-rule log files are created/removed as rules are enabled/disabled. When a rule is disabled or deleted, its log file may remain (stale data is acceptable; cleaned on next config generation).
2. **BR-002**: Time range filtering is done by timestamp comparison during parsing. Lines outside the window are skipped.
3. **BR-003**: If a per-rule log file does not exist or is empty, that rule contributes zero to all metrics (no error).
4. **BR-004**: For "all rules" view, aggregate metrics across all rule log files found in the logs directory.
5. **BR-005**: HTTP error = status >= 400. Stream error = status >= 400 (nginx stream uses 200/400/502/503).
6. **BR-006**: `avg_latency_ms` for HTTP uses `request_time` (total request processing time). For stream, uses `session_time`.
7. **BR-007**: `total_bytes` for HTTP = sum of `body_bytes_sent`. For stream = sum of `bytes_sent + bytes_received`.
8. **BR-008**: Status distribution only applies to HTTP rules. TCP/UDP rules are excluded from the pie chart (they don't have HTTP status codes).
9. **BR-009**: Global `access_log` in combined format is kept for LogsPage backward compatibility.
10. **BR-010**: Auto-refresh interval is 30 seconds. Refreshing resets the timer.

## Edge Cases

- **EC-001**: Rule has no traffic → all metrics are 0, charts show flat zero line
- **EC-002**: Malformed JSON log line → skip line, do not error
- **EC-003**: Log file > 50MB → parse only lines within time window (stream from end)
- **EC-004**: nginx not running → show last known data from log files (no live data disclaimer)
- **EC-005**: Rule deleted but log file remains → "all rules" view still includes orphaned data; per-rule view won't show it (rule not in dropdown)
- **EC-006**: upstream_response_time is "-" (no upstream) → treat as 0ms for latency calc
- **EC-007**: No rules exist → empty state message

## Test Points

| TP-ID | Category | Input | Expected Output |
|-------|----------|-------|-----------------|
| TP-001 | Normal | Request metrics for all rules, 1h range | Summary + 60 time buckets + status distribution returned |
| TP-002 | Normal | Request metrics for specific HTTP rule | Only that rule's data in response |
| TP-003 | Normal | Request metrics for specific TCP rule | Summary with connection count + bytes, no status distribution |
| TP-004 | Normal | Switch time range 1h → 24h | Bucket granularity changes from 1min to 15min |
| TP-005 | Normal | Page auto-refreshes after 30s | New data appears without manual action |
| TP-006 | Boundary | Rule with zero traffic | All metrics 0, empty charts with zero line |
| TP-007 | Boundary | Log file contains malformed JSON line | Line skipped, valid lines still parsed |
| TP-008 | Boundary | upstream_response_time is "-" | Treated as 0ms, no parse error |
| TP-009 | Error | Log file does not exist for a rule | Returns zero metrics, no error |
| TP-010 | Normal | Navigate to /monitor page | Page renders with rule dropdown, time range pills, charts |
| TP-011 | Normal | Sidebar shows "监控"/"Monitor" with Activity icon | Correct label and icon under 代理管理 group |
| TP-012 | Normal | HTTP rule generates traffic → check log file | rule_{id}.access.log created with JSON lines |
| TP-013 | Normal | Stream rule generates traffic → check log file | rule_{id}.access.log created with stream JSON format |
| TP-014 | Combination | Multiple rules, filter by one, then switch to all | Data correctly scoped then aggregated |
| TP-015 | Normal | Charts render: area chart (requests), line chart (latency), pie chart (status) | All three charts visible with correct data |
