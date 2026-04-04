use crate::error::AppError;
use crate::store::models::{CreateProxyRule, UpdateProxyRule};

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

    Ok(())
}

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
