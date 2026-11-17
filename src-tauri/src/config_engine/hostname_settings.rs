//! The settings page's view of the stored `hostname_settings` rows.
//!
//! A row is written only when something has to be remembered for a hostname
//! (today: the HSTS switch). Deleting the last proxy rule that named the
//! hostname does not delete the row: dropping an nginx server block is not a
//! request to forget the hostname's security choices. The price is rows that
//! nothing serves any more — those are *orphans*.
//!
//! This module answers two questions from the same rule list, so the settings
//! page and the cleanup command cannot disagree about what an orphan is:
//!
//! - how many rules still name each stored hostname (zero = orphan), and
//! - which orphans have HSTS on, so cleanup can warn that the browser cache is
//!   not part of what it deletes.

use std::collections::HashMap;

use crate::store::models::{HostnameSettings, ProxyRule};

/// One stored row plus the rule references the settings page shows.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HostnameSettingsEntry {
    #[serde(flatten)]
    pub settings: HostnameSettings,
    /// How many rules name this hostname, enabled or not.
    ///
    /// Disabled rules count: they are still part of the configuration the user
    /// can turn back on, so a row they reference is not garbage.
    pub rule_count: usize,
    /// No rule names this hostname any more, so cleanup may delete the row.
    pub is_orphan: bool,
}

/// Every stored row, sorted by hostname, plus the counts cleanup needs.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HostnameSettingsOverview {
    pub entries: Vec<HostnameSettingsEntry>,
    /// Rows marked `is_orphan` that cleanup would delete right now.
    pub orphan_count: usize,
    /// Orphans whose HSTS switch is on. Deleting one of those rows does not
    /// clear a browser's cached HSTS decision, so cleanup warns about them.
    pub orphan_hsts_count: usize,
}

/// Number of rules naming each hostname.
///
/// Rules without a domain (stream rules, and HTTP rules before a domain is
/// chosen) reference no hostname and are skipped.
fn reference_counts(rules: &[ProxyRule]) -> HashMap<&str, usize> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for rule in rules {
        if let Some(domain) = rule.domain.as_deref() {
            *counts.entry(domain).or_default() += 1;
        }
    }
    counts
}

/// Build the settings page's list from the stored rows and every proxy rule.
pub fn overview(
    settings: &HashMap<String, HostnameSettings>,
    rules: &[ProxyRule],
) -> HostnameSettingsOverview {
    let counts = reference_counts(rules);
    let mut entries: Vec<HostnameSettingsEntry> = settings
        .values()
        .map(|row| {
            let rule_count = counts.get(row.hostname.as_str()).copied().unwrap_or(0);
            HostnameSettingsEntry {
                settings: row.clone(),
                rule_count,
                is_orphan: rule_count == 0,
            }
        })
        .collect();
    entries.sort_by(|left, right| left.settings.hostname.cmp(&right.settings.hostname));

    let orphan_count = entries.iter().filter(|entry| entry.is_orphan).count();
    let orphan_hsts_count = entries
        .iter()
        .filter(|entry| entry.is_orphan && entry.settings.hsts_enabled)
        .count();
    HostnameSettingsOverview {
        entries,
        orphan_count,
        orphan_hsts_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_engine::test_support::http_rule;

    fn stored(rows: &[(&str, bool)]) -> HashMap<String, HostnameSettings> {
        rows.iter()
            .map(|(hostname, hsts)| {
                (
                    hostname.to_string(),
                    HostnameSettings {
                        hostname: hostname.to_string(),
                        hsts_enabled: *hsts,
                        ..HostnameSettings::default()
                    },
                )
            })
            .collect()
    }

    #[test]
    fn every_rule_naming_the_hostname_counts_even_when_disabled() {
        let mut disabled = http_rule("rule-disabled", "app.local");
        disabled.enabled = false;
        let mut stream = http_rule("rule-stream", "");
        stream.domain = None;
        stream.proxy_type = "stream_tcp".to_string();
        let rules = vec![
            http_rule("rule-a", "app.local"),
            disabled,
            stream,
            http_rule("rule-c", "other.local"),
        ];

        let overview = overview(
            &stored(&[("app.local", false), ("other.local", false)]),
            &rules,
        );

        let app = &overview.entries[0];
        assert_eq!(app.settings.hostname, "app.local");
        assert_eq!(
            app.rule_count, 2,
            "a disabled rule still names its hostname"
        );
        assert!(!app.is_orphan);

        let other = &overview.entries[1];
        assert_eq!(other.rule_count, 1);
        assert!(!other.is_orphan);
        assert_eq!(overview.orphan_count, 0);
    }

    #[test]
    fn a_row_no_rule_names_is_an_orphan_and_counts_toward_cleanup() {
        let rules = vec![http_rule("rule-a", "app.local")];
        let overview = overview(
            &stored(&[
                ("app.local", true),
                ("gone.local", true),
                ("old.local", false),
            ]),
            &rules,
        );

        assert_eq!(overview.entries.len(), 3);
        assert_eq!(overview.orphan_count, 2);
        assert_eq!(
            overview.orphan_hsts_count, 1,
            "only the orphan with HSTS on needs the browser-cache warning"
        );

        let gone = overview
            .entries
            .iter()
            .find(|entry| entry.settings.hostname == "gone.local")
            .expect("orphan row");
        assert!(gone.is_orphan);
        assert_eq!(gone.rule_count, 0);
        assert!(gone.settings.hsts_enabled);

        let app = overview
            .entries
            .iter()
            .find(|entry| entry.settings.hostname == "app.local")
            .expect("referenced row");
        assert!(!app.is_orphan);
        assert_eq!(app.rule_count, 1);
    }

    #[test]
    fn the_entries_are_sorted_by_hostname_not_map_order() {
        let overview = overview(
            &stored(&[
                ("zeta.local", false),
                ("alpha.local", false),
                ("middle.local", false),
            ]),
            &[],
        );
        let hostnames: Vec<&str> = overview
            .entries
            .iter()
            .map(|entry| entry.settings.hostname.as_str())
            .collect();
        assert_eq!(hostnames, vec!["alpha.local", "middle.local", "zeta.local"]);
    }

    #[test]
    fn an_empty_database_reports_no_entries_and_no_orphans() {
        let overview = overview(&HashMap::new(), &[]);
        assert!(overview.entries.is_empty());
        assert_eq!(overview.orphan_count, 0);
        assert_eq!(overview.orphan_hsts_count, 0);
    }

    #[test]
    fn the_overview_serialises_the_flat_contract_the_settings_page_reads() {
        let rules = vec![http_rule("rule-a", "app.local")];
        let json = serde_json::to_value(overview(&stored(&[("app.local", true)]), &rules)).unwrap();
        assert_eq!(json["entries"][0]["hostname"], "app.local");
        assert_eq!(json["entries"][0]["hsts_enabled"], true);
        assert_eq!(json["entries"][0]["rule_count"], 1);
        assert_eq!(json["entries"][0]["is_orphan"], false);
        assert_eq!(json["orphan_count"], 0);
        assert_eq!(json["orphan_hsts_count"], 0);
    }
}
