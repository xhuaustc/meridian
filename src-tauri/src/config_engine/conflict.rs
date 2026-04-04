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
