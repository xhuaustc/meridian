//! Default security response headers for generated `server` blocks.
//!
//! The complete list is produced by [`response_headers`] and expanded in exactly
//! one place, `http_config::generate_server_block`. Do not add `add_header`
//! anywhere else: nginx 1.26.2 has no `add_header_inherit merge`, so a
//! `location` that declares any `add_header` silently drops the whole
//! server-level set -- and `nginx -t` still passes. The test helper
//! `test_support::assert_no_add_header_in_locations` guards that invariant.

use crate::store::models::HostnameSettings;

/// One response header to emit in a `server` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseHeader {
    /// HTTP header name.
    pub name: &'static str,
    /// HTTP header value.
    pub value: &'static str,
    /// Whether the header must also be sent for error responses (`always`).
    ///
    /// Without `always`, nginx only sends `add_header` for a short list of
    /// successful statuses, so 502 and 404 pages would silently lose the header.
    pub applies_to_error_responses: bool,
}

/// One row of the header catalogue: which settings column enables it and what
/// it emits.
struct HeaderSpec {
    name: &'static str,
    value: &'static str,
    applies_to_error_responses: bool,
    enabled: fn(&HostnameSettings) -> bool,
}

/// The column-to-header mapping. Adding CSP or Permissions-Policy later means
/// one row here plus one field on `HostnameSettings`, not a hunt through the
/// generator.
const HEADER_SPECS: &[HeaderSpec] = &[
    HeaderSpec {
        name: "X-Content-Type-Options",
        value: "nosniff",
        applies_to_error_responses: true,
        enabled: |settings| settings.sec_nosniff,
    },
    HeaderSpec {
        name: "Referrer-Policy",
        value: "strict-origin-when-cross-origin",
        applies_to_error_responses: true,
        enabled: |settings| settings.sec_referrer_policy,
    },
    HeaderSpec {
        name: "X-Frame-Options",
        value: "SAMEORIGIN",
        applies_to_error_responses: true,
        enabled: |settings| settings.sec_frame_options,
    },
];

/// The complete response-header list for one hostname.
///
/// `settings` is that hostname's `hostname_settings` row. `None` means no row
/// exists, in which case [`HostnameSettings::default`] applies: all three
/// headers on, HSTS off.
pub fn response_headers(settings: Option<&HostnameSettings>) -> Vec<ResponseHeader> {
    let defaults = HostnameSettings::default();
    let settings = settings.unwrap_or(&defaults);
    HEADER_SPECS
        .iter()
        .filter(|spec| (spec.enabled)(settings))
        .map(|spec| ResponseHeader {
            name: spec.name,
            value: spec.value,
            applies_to_error_responses: spec.applies_to_error_responses,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_names(settings: Option<&HostnameSettings>) -> Vec<&'static str> {
        response_headers(settings)
            .into_iter()
            .map(|header| header.name)
            .collect()
    }

    #[test]
    fn a_missing_row_emits_the_three_documented_default_headers() {
        let headers = response_headers(None);
        assert_eq!(
            headers
                .iter()
                .map(|header| (header.name, header.value))
                .collect::<Vec<_>>(),
            vec![
                ("X-Content-Type-Options", "nosniff"),
                ("Referrer-Policy", "strict-origin-when-cross-origin"),
                ("X-Frame-Options", "SAMEORIGIN"),
            ]
        );
    }

    #[test]
    fn every_default_header_also_covers_error_responses() {
        assert!(response_headers(None)
            .iter()
            .all(|header| header.applies_to_error_responses));
    }

    #[test]
    fn each_switch_only_removes_its_own_header() {
        let mut settings = HostnameSettings::default();
        settings.sec_nosniff = false;
        assert_eq!(
            header_names(Some(&settings)),
            vec!["Referrer-Policy", "X-Frame-Options"]
        );

        settings.sec_nosniff = true;
        settings.sec_referrer_policy = false;
        assert_eq!(
            header_names(Some(&settings)),
            vec!["X-Content-Type-Options", "X-Frame-Options"]
        );

        settings.sec_referrer_policy = true;
        settings.sec_frame_options = false;
        assert_eq!(
            header_names(Some(&settings)),
            vec!["X-Content-Type-Options", "Referrer-Policy"]
        );

        settings.sec_nosniff = false;
        assert_eq!(header_names(Some(&settings)), vec!["Referrer-Policy"]);
    }

    #[test]
    fn turning_every_switch_off_emits_no_headers() {
        let settings = HostnameSettings {
            sec_nosniff: false,
            sec_referrer_policy: false,
            sec_frame_options: false,
            ..HostnameSettings::default()
        };
        assert!(response_headers(Some(&settings)).is_empty());
    }

    #[test]
    fn hsts_columns_are_stored_but_hsts_is_not_emitted_yet() {
        let settings = HostnameSettings {
            hsts_enabled: true,
            hsts_max_age: 31_536_000,
            hsts_include_subdomains: true,
            ..HostnameSettings::default()
        };
        assert!(
            !header_names(Some(&settings))
                .iter()
                .any(|name| *name == "Strict-Transport-Security"),
            "this ticket only stores the HSTS columns; a later ticket emits the header"
        );
    }
}
