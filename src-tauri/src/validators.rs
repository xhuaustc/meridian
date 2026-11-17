use crate::error::AppError;
use crate::request_params::{self, RequestParam};
use crate::store::models::{CreateProxyRule, ProxyRule, UpdateProxyRule};

const VALID_PROXY_TYPES: &[&str] = &["http", "stream_tcp", "stream_udp"];
const VALID_TLS_MODES: &[&str] = &["none", "terminate", "passthrough"];

pub fn validate_create_proxy(input: &CreateProxyRule) -> Result<(), AppError> {
    // name: 1-100 chars, not blank
    let name = input.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation(
            "Name must be between 1 and 100 characters".to_string(),
        ));
    }

    // proxy_type: must be http/stream_tcp/stream_udp
    if !VALID_PROXY_TYPES.contains(&input.proxy_type.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid proxy_type '{}'. Must be one of: http, stream_tcp, stream_udp",
            input.proxy_type
        )));
    }

    // listen_port: 1-65535
    if input.listen_port == 0 {
        return Err(AppError::Validation(
            "listen_port must be between 1 and 65535".to_string(),
        ));
    }

    // upstream_port: 1-65535
    if input.upstream_port == 0 {
        return Err(AppError::Validation(
            "upstream_port must be between 1 and 65535".to_string(),
        ));
    }

    // domain: required if proxy_type == "http", must not be empty
    if input.proxy_type == "http" {
        match &input.domain {
            None => {
                return Err(AppError::Validation(
                    "domain is required for HTTP proxy type".to_string(),
                ));
            }
            Some(d) if d.trim().is_empty() => {
                return Err(AppError::Validation(
                    "domain must not be empty for HTTP proxy type".to_string(),
                ));
            }
            _ => {}
        }
    }

    // path_prefix: if present, must start with "/"
    if let Some(ref pp) = input.path_prefix {
        if !pp.is_empty() && !pp.starts_with('/') {
            return Err(AppError::Validation(
                "path_prefix must start with '/'".to_string(),
            ));
        }
    }

    let tls_mode = input.tls_mode.as_deref().unwrap_or("none");

    // tls_mode validation
    if !VALID_TLS_MODES.contains(&tls_mode) {
        return Err(AppError::Validation(format!(
            "Invalid tls_mode '{}'. Must be one of: none, terminate, passthrough",
            tls_mode
        )));
    }

    // if tls_mode == "terminate", certificate_id must be present
    if tls_mode == "terminate" && input.certificate_id.is_none() {
        return Err(AppError::Validation(
            "certificate_id is required when tls_mode is 'terminate'".to_string(),
        ));
    }

    // if tls_mode == "passthrough", certificate_id must NOT be present
    if tls_mode == "passthrough" && input.certificate_id.is_some() {
        return Err(AppError::Validation(
            "certificate_id must not be set when tls_mode is 'passthrough'".to_string(),
        ));
    }

    // websocket only valid for http type
    if input.websocket.unwrap_or(false) && input.proxy_type != "http" {
        return Err(AppError::Validation(
            "websocket is only valid for HTTP proxy type".to_string(),
        ));
    }

    validate_request_params(
        input.client_max_body_size.as_deref(),
        input.client_body_timeout.as_deref(),
        input.proxy_connect_timeout.as_deref(),
        input.proxy_read_timeout.as_deref(),
        input.proxy_send_timeout.as_deref(),
        input.proxy_buffering.as_deref(),
    )?;

    Ok(())
}

/// Request-parameter values a payload would store. `None` means "inherit",
/// which needs no checking; every explicit value is re-checked here even though
/// the form checks it first, because the form is not a trust boundary.
fn validate_request_params(
    client_max_body_size: Option<&str>,
    client_body_timeout: Option<&str>,
    proxy_connect_timeout: Option<&str>,
    proxy_read_timeout: Option<&str>,
    proxy_send_timeout: Option<&str>,
    proxy_buffering: Option<&str>,
) -> Result<(), AppError> {
    request_params::validate_all([
        (RequestParam::ClientMaxBodySize, client_max_body_size),
        (RequestParam::ClientBodyTimeout, client_body_timeout),
        (RequestParam::ProxyConnectTimeout, proxy_connect_timeout),
        (RequestParam::ProxyReadTimeout, proxy_read_timeout),
        (RequestParam::ProxySendTimeout, proxy_send_timeout),
        (RequestParam::ProxyBuffering, proxy_buffering),
    ])
}

#[allow(dead_code)]
pub fn validate_update_proxy(input: &UpdateProxyRule) -> Result<(), AppError> {
    // name: if present, 1-100 chars, not blank
    if let Some(ref name) = input.name {
        let name = name.trim();
        if name.is_empty() || name.len() > 100 {
            return Err(AppError::Validation(
                "Name must be between 1 and 100 characters".to_string(),
            ));
        }
    }

    // proxy_type: if present, must be valid
    if let Some(ref pt) = input.proxy_type {
        if !VALID_PROXY_TYPES.contains(&pt.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid proxy_type '{}'. Must be one of: http, stream_tcp, stream_udp",
                pt
            )));
        }
    }

    // listen_port: if present, 1-65535
    if let Some(port) = input.listen_port {
        if port == 0 {
            return Err(AppError::Validation(
                "listen_port must be between 1 and 65535".to_string(),
            ));
        }
    }

    // upstream_port: if present, 1-65535
    if let Some(port) = input.upstream_port {
        if port == 0 {
            return Err(AppError::Validation(
                "upstream_port must be between 1 and 65535".to_string(),
            ));
        }
    }

    // path_prefix: if present, must start with "/"
    if let Some(ref pp) = input.path_prefix {
        if !pp.is_empty() && !pp.starts_with('/') {
            return Err(AppError::Validation(
                "path_prefix must start with '/'".to_string(),
            ));
        }
    }

    // tls_mode: if present, must be valid
    if let Some(ref tm) = input.tls_mode {
        if !VALID_TLS_MODES.contains(&tm.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid tls_mode '{}'. Must be one of: none, terminate, passthrough",
                tm
            )));
        }
    }

    // Cross-field validation when both tls_mode and certificate_id are provided
    if let Some(ref tm) = input.tls_mode {
        if tm == "terminate" && input.certificate_id.is_none() {
            return Err(AppError::Validation(
                "certificate_id is required when tls_mode is 'terminate'".to_string(),
            ));
        }
        if tm == "passthrough" && input.certificate_id.is_some() {
            return Err(AppError::Validation(
                "certificate_id must not be set when tls_mode is 'passthrough'".to_string(),
            ));
        }
    }

    // websocket only valid for http type
    if let (Some(true), Some(ref pt)) = (input.websocket, &input.proxy_type) {
        if pt != "http" {
            return Err(AppError::Validation(
                "websocket is only valid for HTTP proxy type".to_string(),
            ));
        }
    }

    validate_request_params(
        input.client_max_body_size.as_deref(),
        input.client_body_timeout.as_deref(),
        input.proxy_connect_timeout.as_deref(),
        input.proxy_read_timeout.as_deref(),
        input.proxy_send_timeout.as_deref(),
        input.proxy_buffering.as_deref(),
    )?;

    Ok(())
}

pub fn validate_create_access_list(name: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation(
            "Access list name must be between 1 and 100 characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_ip_cidr(ip_cidr: &str) -> Result<(), AppError> {
    let ip_cidr = ip_cidr.trim();
    if ip_cidr.is_empty() {
        return Err(AppError::Validation(
            "IP/CIDR must not be empty".to_string(),
        ));
    }

    // Support "all" as a special keyword
    if ip_cidr == "all" {
        return Ok(());
    }

    // Basic validation: must look like an IP or CIDR
    // Accept formats: 1.2.3.4, 1.2.3.4/24, ::1, ::1/128, etc.
    if let Some((ip_part, prefix_part)) = ip_cidr.split_once('/') {
        // Validate prefix length
        let prefix: u32 = prefix_part.parse().map_err(|_| {
            AppError::Validation(format!("Invalid CIDR prefix length in '{}'", ip_cidr))
        })?;

        // Determine if IPv4 or IPv6
        if ip_part.contains(':') {
            if prefix > 128 {
                return Err(AppError::Validation(format!(
                    "IPv6 CIDR prefix must be 0-128, got {}",
                    prefix
                )));
            }
        } else {
            if prefix > 32 {
                return Err(AppError::Validation(format!(
                    "IPv4 CIDR prefix must be 0-32, got {}",
                    prefix
                )));
            }
        }
        // Validate IP part by trying to parse it
        validate_ip_address(ip_part, ip_cidr)?;
    } else {
        // Plain IP address
        validate_ip_address(ip_cidr, ip_cidr)?;
    }

    Ok(())
}

fn validate_ip_address(ip: &str, original: &str) -> Result<(), AppError> {
    // Try parsing as IPv4
    if ip.parse::<std::net::Ipv4Addr>().is_ok() {
        return Ok(());
    }
    // Try parsing as IPv6
    if ip.parse::<std::net::Ipv6Addr>().is_ok() {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "Invalid IP address in '{}'",
        original
    )))
}

pub fn validate_host_ip(ip: &str) -> Result<(), AppError> {
    let ip = ip.trim();
    if ip.is_empty() {
        return Err(AppError::Validation(
            "IP address must not be empty".to_string(),
        ));
    }
    if ip.parse::<std::net::Ipv4Addr>().is_err() && ip.parse::<std::net::Ipv6Addr>().is_err() {
        return Err(AppError::Validation(format!("Invalid IP address '{}'", ip)));
    }
    Ok(())
}

pub fn validate_hostname(hostname: &str) -> Result<(), AppError> {
    let hostname = hostname.trim();
    if hostname.is_empty() {
        return Err(AppError::Validation(
            "Hostname must not be empty".to_string(),
        ));
    }
    if hostname.len() > 253 {
        return Err(AppError::Validation(
            "Hostname must not exceed 253 characters".to_string(),
        ));
    }
    let valid = hostname.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    if !valid {
        return Err(AppError::Validation(format!(
            "Invalid hostname format '{}'",
            hostname
        )));
    }
    Ok(())
}

pub fn validate_host_entry(ip: &str, hostname: &str) -> Result<(), AppError> {
    validate_host_ip(ip)?;
    validate_hostname(hostname)?;
    Ok(())
}

/// Bounds for a hostname's HSTS switch.
///
/// The domain is not parsed as a strict DNS name: rule domains may be wildcards
/// or a default server, and this function only has to stop a value that would
/// break the generated nginx config. `max_age` is what browsers store, so the
/// ceiling is one year, and zero is rejected -- turning HSTS off is the
/// toggle's job, not a `max-age=0` special case.
pub fn validate_hsts_settings(domain: &str, max_age: i64) -> Result<(), AppError> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Err(AppError::Validation("domain must not be empty".to_string()));
    }
    if domain.len() > 253 {
        return Err(AppError::Validation(
            "domain must not exceed 253 characters".to_string(),
        ));
    }
    if domain.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!("Invalid domain '{}'", domain)));
    }
    if !(1..=crate::config_engine::hsts::MAX_AGE_LIMIT).contains(&max_age) {
        return Err(AppError::Validation(format!(
            "hsts_max_age must be between 1 and {} seconds",
            crate::config_engine::hsts::MAX_AGE_LIMIT
        )));
    }
    Ok(())
}

pub fn validate_update_proxy_merged(
    input: &UpdateProxyRule,
    existing: &ProxyRule,
) -> Result<(), AppError> {
    // Merge input with existing to produce the "would-be" final state
    let name = input.name.as_deref().unwrap_or(&existing.name);
    let proxy_type = input.proxy_type.as_deref().unwrap_or(&existing.proxy_type);
    let listen_port = input.listen_port.unwrap_or(existing.listen_port);
    let upstream_port = input.upstream_port.unwrap_or(existing.upstream_port);
    let tls_mode = input.tls_mode.as_deref().unwrap_or(&existing.tls_mode);
    let websocket = input.websocket.unwrap_or(existing.websocket);

    // For Option fields that can be explicitly cleared (set to None):
    // input.certificate_id being Some(id) means set, being None means "clear or not provided"
    // Since Rust serde deserializes missing fields as None, we can't distinguish "not provided" from "set to null".
    // However, the frontend ALWAYS sends all fields in UpdateProxyRule (see ProxyForm.tsx).
    // So if certificate_id is None in input, it means "clear it".
    let certificate_id = &input.certificate_id;

    // Validate name
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation(
            "Name must be between 1 and 100 characters".to_string(),
        ));
    }

    // Validate proxy_type
    if !VALID_PROXY_TYPES.contains(&proxy_type) {
        return Err(AppError::Validation(format!(
            "Invalid proxy_type '{}'",
            proxy_type
        )));
    }

    // Validate ports
    if listen_port == 0 {
        return Err(AppError::Validation(
            "listen_port must be between 1 and 65535".to_string(),
        ));
    }
    if upstream_port == 0 {
        return Err(AppError::Validation(
            "upstream_port must be between 1 and 65535".to_string(),
        ));
    }

    // domain required for http
    if proxy_type == "http" {
        let domain = input.domain.as_deref().or(existing.domain.as_deref());
        match domain {
            None | Some("") => {
                return Err(AppError::Validation(
                    "domain is required for HTTP proxy type".to_string(),
                ));
            }
            _ => {}
        }
    }

    // path_prefix must start with /
    let path_prefix = input
        .path_prefix
        .as_deref()
        .or(existing.path_prefix.as_deref());
    if let Some(pp) = path_prefix {
        if !pp.is_empty() && !pp.starts_with('/') {
            return Err(AppError::Validation(
                "path_prefix must start with '/'".to_string(),
            ));
        }
    }

    // tls_mode validation
    if !VALID_TLS_MODES.contains(&tls_mode) {
        return Err(AppError::Validation(format!(
            "Invalid tls_mode '{}'",
            tls_mode
        )));
    }

    // Cross-field: terminate requires certificate
    if tls_mode == "terminate" && certificate_id.is_none() {
        return Err(AppError::Validation(
            "certificate_id is required when tls_mode is 'terminate'".to_string(),
        ));
    }

    // Cross-field: passthrough must not have certificate
    if tls_mode == "passthrough" && certificate_id.is_some() {
        return Err(AppError::Validation(
            "certificate_id must not be set when tls_mode is 'passthrough'".to_string(),
        ));
    }

    // websocket only for http
    if websocket && proxy_type != "http" {
        return Err(AppError::Validation(
            "websocket is only valid for HTTP proxy type".to_string(),
        ));
    }

    validate_request_params(
        input.client_max_body_size.as_deref(),
        input.client_body_timeout.as_deref(),
        input.proxy_connect_timeout.as_deref(),
        input.proxy_read_timeout.as_deref(),
        input.proxy_send_timeout.as_deref(),
        input.proxy_buffering.as_deref(),
    )?;

    Ok(())
}

pub fn validate_create_cert(domain: &str) -> Result<(), AppError> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Err(AppError::Validation(
            "Certificate domain must not be empty".to_string(),
        ));
    }
    if domain.len() > 253 {
        return Err(AppError::Validation(
            "Certificate domain must not exceed 253 characters".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::models::CreateProxyRule;

    fn make_create_input() -> CreateProxyRule {
        CreateProxyRule {
            name: "test".to_string(),
            proxy_type: "http".to_string(),
            listen_port: 80,
            listen_host: None,
            domain: Some("example.com".to_string()),
            path_prefix: None,
            upstream_host: "127.0.0.1".to_string(),
            upstream_port: 3000,
            upstream_scheme: None,
            tls_mode: None,
            certificate_id: None,
            access_list_id: None,
            websocket: None,
            keep_alive: None,
            force_https: None,
            custom_headers: None,
            upstream_targets: None,
            client_max_body_size: None,
            client_body_timeout: None,
            proxy_connect_timeout: None,
            proxy_read_timeout: None,
            proxy_send_timeout: None,
            proxy_buffering: None,
            sort_order: None,
        }
    }

    fn make_update_input() -> UpdateProxyRule {
        UpdateProxyRule {
            name: None,
            proxy_type: None,
            enabled: None,
            listen_port: None,
            listen_host: None,
            domain: None,
            path_prefix: None,
            upstream_host: None,
            upstream_port: None,
            upstream_scheme: None,
            tls_mode: None,
            certificate_id: None,
            access_list_id: None,
            websocket: None,
            keep_alive: None,
            force_https: None,
            custom_headers: None,
            upstream_targets: None,
            client_max_body_size: None,
            client_body_timeout: None,
            proxy_connect_timeout: None,
            proxy_read_timeout: None,
            proxy_send_timeout: None,
            proxy_buffering: None,
            sort_order: None,
        }
    }

    #[test]
    fn test_valid_create_proxy() {
        assert!(validate_create_proxy(&make_create_input()).is_ok());
    }

    #[test]
    fn test_empty_name_rejected() {
        let mut input = make_create_input();
        input.name = "".to_string();
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_name_too_long_rejected() {
        let mut input = make_create_input();
        input.name = "a".repeat(101);
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_invalid_proxy_type() {
        let mut input = make_create_input();
        input.proxy_type = "invalid".to_string();
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_port_zero_rejected() {
        let mut input = make_create_input();
        input.listen_port = 0;
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_http_requires_domain() {
        let mut input = make_create_input();
        input.domain = None;
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_stream_no_domain_ok() {
        let mut input = make_create_input();
        input.proxy_type = "stream_tcp".to_string();
        input.domain = None;
        assert!(validate_create_proxy(&input).is_ok());
    }

    #[test]
    fn test_terminate_requires_cert() {
        let mut input = make_create_input();
        input.tls_mode = Some("terminate".to_string());
        input.certificate_id = None;
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_passthrough_rejects_cert() {
        let mut input = make_create_input();
        input.tls_mode = Some("passthrough".to_string());
        input.certificate_id = Some("cert-1".to_string());
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_websocket_only_http() {
        let mut input = make_create_input();
        input.proxy_type = "stream_tcp".to_string();
        input.domain = None;
        input.websocket = Some(true);
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_path_prefix_must_start_with_slash() {
        let mut input = make_create_input();
        input.path_prefix = Some("api".to_string());
        assert!(validate_create_proxy(&input).is_err());
    }

    #[test]
    fn test_path_prefix_with_slash_ok() {
        let mut input = make_create_input();
        input.path_prefix = Some("/api".to_string());
        assert!(validate_create_proxy(&input).is_ok());
    }

    // IP/CIDR validation tests
    #[test]
    fn test_valid_ipv4() {
        assert!(validate_ip_cidr("192.168.1.1").is_ok());
    }

    #[test]
    fn test_valid_ipv4_cidr() {
        assert!(validate_ip_cidr("10.0.0.0/8").is_ok());
    }

    #[test]
    fn test_valid_ipv6() {
        assert!(validate_ip_cidr("::1").is_ok());
    }

    #[test]
    fn test_valid_ipv6_cidr() {
        assert!(validate_ip_cidr("fe80::/10").is_ok());
    }

    #[test]
    fn test_all_keyword() {
        assert!(validate_ip_cidr("all").is_ok());
    }

    #[test]
    fn test_invalid_ip() {
        assert!(validate_ip_cidr("not-an-ip").is_err());
    }

    #[test]
    fn test_ipv4_cidr_too_large() {
        assert!(validate_ip_cidr("10.0.0.0/33").is_err());
    }

    #[test]
    fn test_ipv6_cidr_too_large() {
        assert!(validate_ip_cidr("::1/129").is_err());
    }

    // Hostname validation
    #[test]
    fn test_valid_hostname() {
        assert!(validate_hostname("example.com").is_ok());
    }

    #[test]
    fn test_hostname_too_long() {
        assert!(validate_hostname(&"a".repeat(254)).is_err());
    }

    #[test]
    fn test_hostname_invalid_chars() {
        assert!(validate_hostname("exam ple.com").is_err());
    }

    #[test]
    fn test_hostname_leading_dash() {
        assert!(validate_hostname("-example.com").is_err());
    }

    // Host entry validation
    #[test]
    fn test_valid_host_entry() {
        assert!(validate_host_entry("127.0.0.1", "localhost").is_ok());
    }

    #[test]
    fn test_invalid_host_ip() {
        assert!(validate_host_entry("not-ip", "localhost").is_err());
    }

    // Request parameter validation
    #[test]
    fn test_request_parameters_accept_values_with_units() {
        let mut input = make_create_input();
        input.client_max_body_size = Some("2g".to_string());
        input.client_body_timeout = Some("300s".to_string());
        input.proxy_connect_timeout = Some("10s".to_string());
        input.proxy_read_timeout = Some("3600s".to_string());
        input.proxy_send_timeout = Some("500ms".to_string());
        input.proxy_buffering = Some("off".to_string());
        assert!(validate_create_proxy(&input).is_ok());
    }

    #[test]
    fn test_request_parameters_reject_a_bare_number_and_name_the_field() {
        let mut input = make_create_input();
        input.client_max_body_size = Some("2".to_string());
        let message = validate_create_proxy(&input).unwrap_err().to_string();
        assert!(message.contains("client_max_body_size"), "{message}");
        assert!(message.contains("k, m or g"), "{message}");
        assert!(message.contains("'2'"), "{message}");
    }

    #[test]
    fn test_request_parameters_reject_a_non_toggle_buffering_value() {
        let mut input = make_create_input();
        input.proxy_buffering = Some("0".to_string());
        let message = validate_create_proxy(&input).unwrap_err().to_string();
        assert!(message.contains("proxy_buffering"), "{message}");
        assert!(message.contains("on or off"), "{message}");
    }

    #[test]
    fn test_update_validation_checks_request_parameters_too() {
        let existing = crate::config_engine::test_support::http_rule("rule-1", "app.local");
        let mut input = make_update_input();
        input.proxy_read_timeout = Some("30".to_string());
        let message = validate_update_proxy_merged(&input, &existing)
            .unwrap_err()
            .to_string();
        assert!(message.contains("proxy_read_timeout"), "{message}");
    }

    #[test]
    fn test_hsts_settings_accept_the_default_and_the_one_year_ceiling() {
        assert!(validate_hsts_settings("app.local", 300).is_ok());
        assert!(
            validate_hsts_settings("app.local", crate::config_engine::hsts::MAX_AGE_LIMIT).is_ok()
        );
    }

    #[test]
    fn test_hsts_settings_reject_zero_negative_and_longer_than_one_year() {
        for max_age in [0, -1, crate::config_engine::hsts::MAX_AGE_LIMIT + 1] {
            let message = validate_hsts_settings("app.local", max_age)
                .unwrap_err()
                .to_string();
            assert!(message.contains("hsts_max_age"), "{message}");
        }
    }

    #[test]
    fn test_hsts_settings_reject_an_empty_or_whitespace_domain() {
        assert!(validate_hsts_settings("", 300).is_err());
        assert!(validate_hsts_settings("   ", 300).is_err());
        assert!(validate_hsts_settings("app local", 300).is_err());
    }
}
