//! Default security response headers for generated `server` blocks.
//!
//! The complete list is produced by [`response_headers`] and expanded in exactly
//! one place, `http_config::generate_server_block`. Do not add `add_header`
//! anywhere else: nginx 1.26.2 has no `add_header_inherit merge`, so a
//! `location` that declares any `add_header` silently drops the whole
//! server-level set -- and `nginx -t` still passes. The test helper
//! `test_support::assert_no_add_header_in_locations` guards that invariant.
//!
//! HSTS lives in the same catalogue, but unlike the three default headers it is
//! conditional on two things at once: the hostname's row must turn it on, and
//! the `server` block must terminate TLS. A rule that listens on 443 with
//! `tls_mode = none` therefore gets no HSTS -- TLS is decided by the mode, never
//! by the port number.

use crate::store::models::HostnameSettings;

/// The server block the header list is produced for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseContext {
    /// True when this `server` block terminates TLS (`listen ... ssl`).
    pub tls_terminated: bool,
}

/// One response header to emit in a `server` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseHeader {
    /// HTTP header name.
    pub name: &'static str,
    /// HTTP header value.
    pub value: String,
    /// Whether the header must also be sent for error responses (`always`).
    ///
    /// Without `always`, nginx only sends `add_header` for a short list of
    /// successful statuses, so 502 and 404 pages would silently lose the header.
    pub applies_to_error_responses: bool,
    /// Whether the value must be quoted in the generated config.
    ///
    /// HSTS values contain `;` between directives, which nginx would otherwise
    /// read as the end of the statement; quoting keeps the whole value one token.
    pub quote_value: bool,
}

/// One row of the header catalogue: what settings column enables it and what
/// it emits.
struct HeaderSpec {
    name: &'static str,
    applies_to_error_responses: bool,
    quote_value: bool,
    /// The header value, or `None` when this row does not enable the header.
    value: fn(&HostnameSettings, ResponseContext) -> Option<String>,
}

/// The column-to-header mapping. Adding CSP or Permissions-Policy later means
/// one row here plus one field on `HostnameSettings`, not a hunt through the
/// generator.
const HEADER_SPECS: &[HeaderSpec] = &[
    HeaderSpec {
        name: "X-Content-Type-Options",
        applies_to_error_responses: true,
        quote_value: false,
        value: |settings, _| settings.sec_nosniff.then(|| "nosniff".to_string()),
    },
    HeaderSpec {
        name: "Referrer-Policy",
        applies_to_error_responses: true,
        quote_value: false,
        value: |settings, _| {
            settings
                .sec_referrer_policy
                .then(|| "strict-origin-when-cross-origin".to_string())
        },
    },
    HeaderSpec {
        name: "X-Frame-Options",
        applies_to_error_responses: true,
        quote_value: false,
        value: |settings, _| settings.sec_frame_options.then(|| "SAMEORIGIN".to_string()),
    },
    HeaderSpec {
        name: "Strict-Transport-Security",
        applies_to_error_responses: true,
        quote_value: true,
        value: |settings, context| hsts_value(settings, context.tls_terminated),
    },
];

/// The `Strict-Transport-Security` value for a hostname, or `None` when the
/// server block must not send it.
///
/// The value is `max-age=<seconds>` and, when the row asks for it,
/// `; includeSubDomains`. There is deliberately no `preload` option: a preload
/// is a one-way commitment to the browser vendor's list, and this product keeps
/// the switch undoable.
fn hsts_value(settings: &HostnameSettings, tls_terminated: bool) -> Option<String> {
    if !tls_terminated || !settings.hsts_enabled {
        return None;
    }
    // The column is validated on write; clamping keeps a hand-edited database
    // from producing a value nginx would reject.
    let mut value = format!("max-age={}", settings.hsts_max_age.max(0));
    if settings.hsts_include_subdomains {
        value.push_str("; includeSubDomains");
    }
    Some(value)
}

/// The complete response-header list for one hostname.
///
/// `settings` is that hostname's `hostname_settings` row. `None` means no row
/// exists, in which case [`HostnameSettings::default`] applies: all three
/// default headers on, HSTS off. `context` decides whether the HSTS row may
/// take effect at all.
pub fn response_headers(
    settings: Option<&HostnameSettings>,
    context: ResponseContext,
) -> Vec<ResponseHeader> {
    let defaults = HostnameSettings::default();
    let settings = settings.unwrap_or(&defaults);
    HEADER_SPECS
        .iter()
        .filter_map(|spec| {
            let value = (spec.value)(settings, context)?;
            Some(ResponseHeader {
                name: spec.name,
                value,
                applies_to_error_responses: spec.applies_to_error_responses,
                quote_value: spec.quote_value,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tls() -> ResponseContext {
        ResponseContext {
            tls_terminated: true,
        }
    }

    fn plain() -> ResponseContext {
        ResponseContext {
            tls_terminated: false,
        }
    }

    fn header_names(
        settings: Option<&HostnameSettings>,
        context: ResponseContext,
    ) -> Vec<&'static str> {
        response_headers(settings, context)
            .into_iter()
            .map(|header| header.name)
            .collect()
    }

    fn header_value(
        settings: Option<&HostnameSettings>,
        context: ResponseContext,
        name: &str,
    ) -> Option<String> {
        response_headers(settings, context)
            .into_iter()
            .find(|header| header.name == name)
            .map(|header| header.value)
    }

    fn hsts_on() -> HostnameSettings {
        HostnameSettings {
            hsts_enabled: true,
            ..HostnameSettings::default()
        }
    }

    #[test]
    fn a_missing_row_emits_the_three_documented_default_headers() {
        let headers = response_headers(None, tls());
        assert_eq!(
            headers
                .iter()
                .map(|header| (header.name, header.value.as_str()))
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
        assert!(response_headers(None, tls())
            .iter()
            .all(|header| header.applies_to_error_responses));
        assert!(response_headers(Some(&hsts_on()), tls())
            .iter()
            .all(|header| header.applies_to_error_responses));
    }

    #[test]
    fn each_switch_only_removes_its_own_header() {
        let mut settings = HostnameSettings::default();
        settings.sec_nosniff = false;
        assert_eq!(
            header_names(Some(&settings), tls()),
            vec!["Referrer-Policy", "X-Frame-Options"]
        );

        settings.sec_nosniff = true;
        settings.sec_referrer_policy = false;
        assert_eq!(
            header_names(Some(&settings), tls()),
            vec!["X-Content-Type-Options", "X-Frame-Options"]
        );

        settings.sec_referrer_policy = true;
        settings.sec_frame_options = false;
        assert_eq!(
            header_names(Some(&settings), tls()),
            vec!["X-Content-Type-Options", "Referrer-Policy"]
        );

        settings.sec_nosniff = false;
        assert_eq!(
            header_names(Some(&settings), tls()),
            vec!["Referrer-Policy"]
        );
    }

    #[test]
    fn turning_every_switch_off_emits_no_headers() {
        let settings = HostnameSettings {
            hsts_enabled: false,
            sec_nosniff: false,
            sec_referrer_policy: false,
            sec_frame_options: false,
            ..HostnameSettings::default()
        };
        assert!(response_headers(Some(&settings), tls()).is_empty());
    }

    #[test]
    fn hsts_is_absent_without_a_row_even_on_a_tls_server() {
        assert!(
            !header_names(None, tls())
                .iter()
                .any(|name| *name == "Strict-Transport-Security"),
            "HSTS defaults to off; only the hostname's row can turn it on"
        );
    }

    #[test]
    fn hsts_only_appears_on_tls_terminating_server_blocks() {
        let settings = hsts_on();
        assert_eq!(
            header_value(Some(&settings), tls(), "Strict-Transport-Security").as_deref(),
            Some("max-age=300"),
            "the default max-age is five minutes so a mistake stays undoable"
        );
        assert_eq!(
            header_value(Some(&settings), plain(), "Strict-Transport-Security"),
            None,
            "a plain HTTP server block must never advertise HSTS"
        );
    }

    #[test]
    fn hsts_uses_the_configured_max_age_and_include_subdomains() {
        let settings = HostnameSettings {
            hsts_enabled: true,
            hsts_max_age: 31_536_000,
            hsts_include_subdomains: true,
            ..HostnameSettings::default()
        };
        assert_eq!(
            header_value(Some(&settings), tls(), "Strict-Transport-Security").as_deref(),
            Some("max-age=31536000; includeSubDomains")
        );
    }

    #[test]
    fn hsts_never_advertises_preload() {
        let settings = HostnameSettings {
            hsts_enabled: true,
            hsts_include_subdomains: true,
            ..HostnameSettings::default()
        };
        let value =
            header_value(Some(&settings), tls(), "Strict-Transport-Security").expect("HSTS value");
        assert!(
            !value.contains("preload"),
            "preload is a one-way commitment and is not offered: {value}"
        );
    }

    #[test]
    fn hsts_value_has_to_be_quoted_because_it_contains_a_semicolon() {
        let headers = response_headers(Some(&hsts_on()), tls());
        let hsts = headers
            .iter()
            .find(|header| header.name == "Strict-Transport-Security")
            .expect("HSTS header");
        assert!(hsts.quote_value);
        assert!(headers
            .iter()
            .filter(|header| header.name != "Strict-Transport-Security")
            .all(|header| !header.quote_value));
    }
}
