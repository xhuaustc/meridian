# FEAT-002: Proxy Monitoring — Tasks

## Dependencies

```
TASK-001 (nginx log format) ← TASK-002 (per-rule log directives)
TASK-003 (log parser) ← TASK-004 (aggregator + command)
TASK-004 ← TASK-006 (frontend page)
TASK-005 (recharts + types) ← TASK-006
```

## Task Table

| ID | Title | Spec Ref | Est | Deps | Status | AI-Auto |
|----|-------|----------|-----|------|--------|---------|
| TASK-001 | Nginx custom log_format in main_config | spec.md §Nginx Log Format | 20min | — | Pending | Yes |
| TASK-002 | Per-rule access_log in http_config + stream_config | spec.md §Per-Rule Log Files | 30min | TASK-001 | Pending | Yes |
| TASK-003 | Rust log parser module (JSON line parsing) | spec.md §Interface, design.md §Log Parser | 45min | — | Pending | Yes |
| TASK-004 | Rust aggregator + `get_proxy_metrics` command | spec.md §Interface, design.md §Aggregation | 45min | TASK-003 | Pending | Yes |
| TASK-005 | Install recharts + TypeScript types + i18n keys | spec.md §Interface | 15min | — | Pending | Yes |
| TASK-006 | MonitorPage UI with charts + sidebar nav | spec.md §Use Cases, design.md §Frontend | 60min | TASK-004, TASK-005 | Pending | Yes |
