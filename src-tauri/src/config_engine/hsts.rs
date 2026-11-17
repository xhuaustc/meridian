//! HSTS is a hostname-level switch that only TLS-terminating servers emit.
//!
//! The `hostname_settings` row decides whether a hostname sends
//! `Strict-Transport-Security`; [`crate::config_engine::security_headers`]
//! produces the header value. Because the setting belongs to the hostname and
//! not to a rule, several rules can share it, and only one of them may present
//! the toggle. That rule is the *owner*: the enabled HTTP rule for the hostname
//! with the lowest TLS port, tie-broken by rule id. It mirrors the "lowest port
//! owns the hostname-wide artefact" rule used for the synthesised port-80
//! redirect, and it is stable while rules are edited.
//!
//! The form asks the backend for this answer instead of guessing: a rule that
//! is not the owner shows the read-only hint "already enabled by rule X" rather
//! than offering a second toggle for the same hostname.

use std::collections::HashMap;

use crate::store::models::{HostnameSettings, ProxyRule};

/// Longest `max-age` the form and the backend accept, in seconds (one year).
///
/// HSTS is stored in browsers for the full window and cannot be revoked, so the
/// product keeps the ceiling deliberately low; anything longer belongs in a
/// hand-written nginx config.
pub const MAX_AGE_LIMIT: i64 = 31_536_000;

/// The enabled rule whose form owns a hostname's HSTS switch.
///
/// Only rules that actually emit the header can own it: HTTP rules that
/// terminate TLS. Disabled rules never reach generation and therefore never
/// own anything (callers pass `proxy_repo::list_enabled`).
pub fn owner_rule<'a>(rules: &'a [ProxyRule], domain: &str) -> Option<&'a ProxyRule> {
    rules
        .iter()
        .filter(|rule| {
            rule.proxy_type == "http"
                && rule.tls_mode == "terminate"
                && rule.domain.as_deref() == Some(domain)
        })
        .min_by(|left, right| {
            (left.listen_port, left.id.as_str()).cmp(&(right.listen_port, right.id.as_str()))
        })
}

/// What the rule form needs to render the HSTS controls without guessing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HstsCheck {
    /// Whether the hostname's row has HSTS on.
    pub enabled: bool,
    /// `max-age` in seconds as stored (the row default is 300).
    pub max_age: i64,
    /// Whether the row asks for `includeSubDomains`.
    pub include_subdomains: bool,
    /// The rule whose form owns the switch; `None` when no TLS-terminating rule
    /// serves this hostname yet, so the rule being edited (or created) is the
    /// owner.
    pub owner_rule_id: Option<String>,
    /// The owner's display name, for the read-only hint.
    pub owner_rule_name: Option<String>,
}

/// Read the hostname's HSTS state and the rule that owns the toggle.
pub fn check(rules: &[ProxyRule], settings: Option<&HostnameSettings>, domain: &str) -> HstsCheck {
    let defaults = HostnameSettings::default();
    let settings = settings.unwrap_or(&defaults);
    let owner = owner_rule(rules, domain);
    HstsCheck {
        enabled: settings.hsts_enabled,
        max_age: settings.hsts_max_age,
        include_subdomains: settings.hsts_include_subdomains,
        owner_rule_id: owner.map(|rule| rule.id.clone()),
        owner_rule_name: owner.map(|rule| rule.name.clone()),
    }
}

/// Hostnames whose HSTS switch is on, sorted.
///
/// The rule list uses this to flag certificates that are about to expire on an
/// HSTS-locked hostname; the settings page will read the full rows.
pub fn enabled_hostnames(settings: &HashMap<String, HostnameSettings>) -> Vec<String> {
    let mut hostnames: Vec<String> = settings
        .values()
        .filter(|row| row.hsts_enabled)
        .map(|row| row.hostname.clone())
        .collect();
    hostnames.sort();
    hostnames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_engine::test_support::http_rule;

    fn tls_rule(id: &str, port: u16, domain: &str) -> ProxyRule {
        let mut rule = http_rule(id, domain);
        rule.listen_port = port;
        rule.tls_mode = "terminate".to_string();
        rule
    }

    #[test]
    fn the_owner_is_the_lowest_tls_port_for_the_hostname() {
        let rules = vec![
            tls_rule("rule-b", 8443, "app.local"),
            tls_rule("rule-a", 443, "app.local"),
            tls_rule("rule-other", 443, "other.local"),
        ];
        let owner = owner_rule(&rules, "app.local").expect("an owner");
        assert_eq!(owner.id, "rule-a");
        assert_eq!(owner.listen_port, 443);
    }

    #[test]
    fn only_rules_that_can_emit_hsts_can_own_the_switch() {
        let mut plain = http_rule("rule-plain", "app.local");
        plain.listen_port = 80;
        let mut passthrough = tls_rule("rule-passthrough", 443, "app.local");
        passthrough.tls_mode = "passthrough".to_string();
        let mut stream = http_rule("rule-stream", "app.local");
        stream.proxy_type = "stream_tcp".to_string();
        stream.domain = None;

        assert!(
            owner_rule(&[plain, passthrough, stream], "app.local").is_none(),
            "a rule that cannot emit HSTS must not own its toggle"
        );
    }

    #[test]
    fn equal_ports_are_broken_by_rule_id_so_the_owner_cannot_flap() {
        let rules = vec![
            tls_rule("rule-z", 443, "app.local"),
            tls_rule("rule-a", 443, "app.local"),
        ];
        assert_eq!(
            owner_rule(&rules, "app.local").map(|rule| rule.id.as_str()),
            Some("rule-a")
        );
    }

    #[test]
    fn a_hostname_without_a_row_reports_the_safe_defaults() {
        let rules = vec![tls_rule("rule-a", 443, "app.local")];
        let check = check(&rules, None, "app.local");
        assert!(!check.enabled);
        assert_eq!(check.max_age, 300);
        assert!(!check.include_subdomains);
        assert_eq!(check.owner_rule_id.as_deref(), Some("rule-a"));
        assert_eq!(check.owner_rule_name.as_deref(), Some("test app.local"));
    }

    #[test]
    fn the_check_reports_the_stored_row_and_a_missing_owner() {
        let settings = HostnameSettings {
            hostname: "app.local".to_string(),
            hsts_enabled: true,
            hsts_max_age: 600,
            hsts_include_subdomains: true,
            ..HostnameSettings::default()
        };
        let check = check(&[], Some(&settings), "app.local");
        assert!(check.enabled);
        assert_eq!(check.max_age, 600);
        assert!(check.include_subdomains);
        assert_eq!(check.owner_rule_id, None);
        assert_eq!(check.owner_rule_name, None);
    }

    #[test]
    fn enabled_hostnames_lists_only_the_on_rows_in_order() {
        let settings = HashMap::from([
            (
                "off.local".to_string(),
                HostnameSettings {
                    hostname: "off.local".to_string(),
                    ..HostnameSettings::default()
                },
            ),
            (
                "zeta.local".to_string(),
                HostnameSettings {
                    hostname: "zeta.local".to_string(),
                    hsts_enabled: true,
                    ..HostnameSettings::default()
                },
            ),
            (
                "alpha.local".to_string(),
                HostnameSettings {
                    hostname: "alpha.local".to_string(),
                    hsts_enabled: true,
                    ..HostnameSettings::default()
                },
            ),
        ]);
        assert_eq!(
            enabled_hostnames(&settings),
            vec!["alpha.local".to_string(), "zeta.local".to_string()]
        );
        assert!(enabled_hostnames(&HashMap::new()).is_empty());
    }

    #[test]
    fn the_check_serialises_the_contract_the_form_reads() {
        let rules = vec![tls_rule("rule-a", 443, "app.local")];
        let json = serde_json::to_value(check(&rules, None, "app.local")).unwrap();
        assert_eq!(json["enabled"], false);
        assert_eq!(json["max_age"], 300);
        assert_eq!(json["include_subdomains"], false);
        assert_eq!(json["owner_rule_id"], "rule-a");
        assert_eq!(json["owner_rule_name"], "test app.local");
    }
}
