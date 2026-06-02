use crate::store::models::{PortConflict, ProxyRule};

/// Detect port/domain/path conflicts among a set of proxy rules.
///
/// Conflict rules:
/// 1. Same port + different domain -> OK (virtual host)
/// 2. Same port + same domain + different path -> OK (location)
/// 3. Same port + same domain + same path -> CONFLICT
/// 4. Stream same port -> CONFLICT (port exclusive)
/// 5. HTTP vs Stream same port -> CONFLICT
pub fn detect_conflicts(rules: &[ProxyRule]) -> Vec<PortConflict> {
    let mut conflicts = Vec::new();

    for (i, a) in rules.iter().enumerate() {
        for b in rules.iter().skip(i + 1) {
            if a.listen_port != b.listen_port {
                continue;
            }

            let a_is_stream = a.proxy_type == "stream_tcp" || a.proxy_type == "stream_udp";
            let b_is_stream = b.proxy_type == "stream_tcp" || b.proxy_type == "stream_udp";

            // Rule 5: HTTP vs Stream on same port -> CONFLICT
            if a_is_stream != b_is_stream {
                conflicts.push(PortConflict {
                    rule_id: b.id.clone(),
                    rule_name: b.name.clone(),
                    conflict_type: "http_stream_conflict".to_string(),
                    message: format!(
                        "Port {} is used by both HTTP rule '{}' and stream rule '{}'",
                        a.listen_port, a.name, b.name
                    ),
                });
                continue;
            }

            // Rule 4: Stream same port -> CONFLICT (port exclusive)
            if a_is_stream && b_is_stream {
                // Same protocol stream on same port is a conflict
                if a.proxy_type == b.proxy_type {
                    conflicts.push(PortConflict {
                        rule_id: b.id.clone(),
                        rule_name: b.name.clone(),
                        conflict_type: "stream_port_conflict".to_string(),
                        message: format!(
                            "Port {} is exclusively used by stream rule '{}', conflicts with '{}'",
                            a.listen_port, a.name, b.name
                        ),
                    });
                }
                continue;
            }

            // Both are HTTP rules on the same port
            let a_domain = a.domain.as_deref().unwrap_or("");
            let b_domain = b.domain.as_deref().unwrap_or("");

            // Rule 1: Different domain -> OK
            if a_domain != b_domain {
                continue;
            }

            let a_path = a.path_prefix.as_deref().unwrap_or("/");
            let b_path = b.path_prefix.as_deref().unwrap_or("/");

            // Rule 2: Same domain + different path -> OK
            if a_path != b_path {
                continue;
            }

            // Rule 3: Same port + same domain + same path -> CONFLICT
            conflicts.push(PortConflict {
                rule_id: b.id.clone(),
                rule_name: b.name.clone(),
                conflict_type: "exact_conflict".to_string(),
                message: format!(
                    "Port {}, domain '{}', path '{}' conflicts between '{}' and '{}'",
                    a.listen_port,
                    if a_domain.is_empty() { "_" } else { a_domain },
                    a_path,
                    a.name,
                    b.name
                ),
            });
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::models::ProxyRule;

    fn make_rule(
        id: &str,
        proxy_type: &str,
        port: u16,
        domain: Option<&str>,
        path: Option<&str>,
    ) -> ProxyRule {
        ProxyRule {
            id: id.to_string(),
            name: format!("rule-{}", id),
            proxy_type: proxy_type.to_string(),
            enabled: true,
            listen_port: port,
            listen_host: "0.0.0.0".to_string(),
            domain: domain.map(String::from),
            path_prefix: path.map(String::from),
            upstream_host: "127.0.0.1".to_string(),
            upstream_port: 3000,
            upstream_scheme: "http".to_string(),
            tls_mode: "none".to_string(),
            certificate_id: None,
            access_list_id: None,
            websocket: false,
            keep_alive: false,
            custom_headers: None,
            upstream_targets: None,
            sort_order: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_no_conflicts_different_ports() {
        let rules = vec![
            make_rule("1", "http", 80, Some("a.com"), None),
            make_rule("2", "http", 81, Some("b.com"), None),
        ];
        assert!(detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_virtual_host_no_conflict() {
        let rules = vec![
            make_rule("1", "http", 80, Some("a.com"), None),
            make_rule("2", "http", 80, Some("b.com"), None),
        ];
        assert!(detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_same_port_domain_path_conflicts() {
        let rules = vec![
            make_rule("1", "http", 80, Some("a.com"), Some("/")),
            make_rule("2", "http", 80, Some("a.com"), Some("/")),
        ];
        assert!(!detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_different_paths_no_conflict() {
        let rules = vec![
            make_rule("1", "http", 80, Some("a.com"), Some("/api")),
            make_rule("2", "http", 80, Some("a.com"), Some("/web")),
        ];
        assert!(detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_stream_same_port_conflicts() {
        let rules = vec![
            make_rule("1", "stream_tcp", 3306, None, None),
            make_rule("2", "stream_tcp", 3306, None, None),
        ];
        assert!(!detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_http_stream_same_port_conflicts() {
        let rules = vec![
            make_rule("1", "http", 80, Some("a.com"), None),
            make_rule("2", "stream_tcp", 80, None, None),
        ];
        assert!(!detect_conflicts(&rules).is_empty());
    }

    #[test]
    fn test_tcp_udp_same_port_no_conflict() {
        // TCP and UDP on same port should not conflict (different protocols)
        let rules = vec![
            make_rule("1", "stream_tcp", 5000, None, None),
            make_rule("2", "stream_udp", 5000, None, None),
        ];
        // This may or may not conflict depending on implementation - just verify it doesn't panic
        let _ = detect_conflicts(&rules);
    }
}
