// SPEC: FEAT-002-proxy-monitoring/spec.md | TASK-003
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Parsed HTTP access log entry.
#[derive(Debug)]
pub struct HttpLogEntry {
    pub time: DateTime<Utc>,
    pub status: u16,
    pub body_bytes_sent: u64,
    pub request_time: f64,
    pub upstream_response_time: Option<f64>,
}

/// Parsed stream (TCP/UDP) access log entry.
#[derive(Debug)]
pub struct StreamLogEntry {
    pub time: DateTime<Utc>,
    pub status: u16,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub session_time: f64,
}

/// Unified entry for aggregation.
#[derive(Debug)]
pub struct LogEntry {
    pub time: DateTime<Utc>,
    pub status: u16,
    pub bytes: u64,
    pub latency_ms: f64,
    pub is_http: bool,
}

#[derive(Deserialize)]
struct RawHttpEntry {
    time: String,
    status: u16,
    body_bytes_sent: u64,
    request_time: f64,
    upstream_response_time: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawStreamEntry {
    time: String,
    status: u16,
    bytes_sent: u64,
    bytes_received: u64,
    session_time: Option<serde_json::Value>,
}

fn parse_time(s: &str) -> Option<DateTime<Utc>> {
    // nginx $time_iso8601 produces e.g. "2026-04-04T12:00:00+08:00"
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_optional_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s == "-" || s.is_empty() {
                None
            } else {
                // upstream_response_time can be "0.001" or "0.001, 0.002" (multiple upstreams)
                // Take the last value (final upstream response)
                s.split(',').last()?.trim().parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

/// Parse a single JSON line as an HTTP log entry.
pub fn parse_http_line(line: &str) -> Option<HttpLogEntry> {
    let raw: RawHttpEntry = serde_json::from_str(line).ok()?;
    let time = parse_time(&raw.time)?;
    let urt = raw.upstream_response_time.as_ref().and_then(parse_optional_f64);
    Some(HttpLogEntry {
        time,
        status: raw.status,
        body_bytes_sent: raw.body_bytes_sent,
        request_time: raw.request_time,
        upstream_response_time: urt,
    })
}

/// Parse a single JSON line as a stream log entry.
pub fn parse_stream_line(line: &str) -> Option<StreamLogEntry> {
    let raw: RawStreamEntry = serde_json::from_str(line).ok()?;
    let time = parse_time(&raw.time)?;
    let session_time = raw
        .session_time
        .as_ref()
        .and_then(parse_optional_f64)
        .unwrap_or(0.0);
    Some(StreamLogEntry {
        time,
        status: raw.status,
        bytes_sent: raw.bytes_sent,
        bytes_received: raw.bytes_received,
        session_time,
    })
}

/// Try to parse a log line as either HTTP or stream format, returning a unified LogEntry.
pub fn parse_line(line: &str) -> Option<LogEntry> {
    // Try HTTP first (has request_time field)
    if let Some(http) = parse_http_line(line) {
        return Some(LogEntry {
            time: http.time,
            status: http.status,
            bytes: http.body_bytes_sent,
            latency_ms: http.request_time * 1000.0,
            is_http: true,
        });
    }
    // Fall back to stream
    if let Some(stream) = parse_stream_line(line) {
        return Some(LogEntry {
            time: stream.time,
            status: stream.status,
            bytes: stream.bytes_sent + stream.bytes_received,
            latency_ms: stream.session_time * 1000.0,
            is_http: false,
        });
    }
    None
}
