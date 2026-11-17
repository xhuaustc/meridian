//! The six per-rule request parameters: their units, the global defaults a rule
//! inherits, and the presets the rule form offers.
//!
//! Every name here is the nginx directive the parameter produces, so the column,
//! the form row and the generated config cannot drift into separate
//! vocabularies. A stored `NULL` means "inherit the global default"; a stored
//! `0` means "unlimited" for the five parameters where nginx accepts it.
//!
//! Unit rules live here and nowhere else. The form mirrors them for instant
//! feedback, and the backend validates every value again before storing it.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// One of the six parameters a rule can override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestParam {
    ClientMaxBodySize,
    ClientBodyTimeout,
    ProxyConnectTimeout,
    ProxyReadTimeout,
    ProxySendTimeout,
    ProxyBuffering,
}

impl RequestParam {
    /// Every parameter, in the order the form rows and the generated location
    /// directives list them.
    pub const ALL: [RequestParam; 6] = [
        RequestParam::ClientMaxBodySize,
        RequestParam::ClientBodyTimeout,
        RequestParam::ProxyConnectTimeout,
        RequestParam::ProxyReadTimeout,
        RequestParam::ProxySendTimeout,
        RequestParam::ProxyBuffering,
    ];

    /// The nginx directive this parameter produces; also the column name.
    pub fn directive(self) -> &'static str {
        match self {
            RequestParam::ClientMaxBodySize => "client_max_body_size",
            RequestParam::ClientBodyTimeout => "client_body_timeout",
            RequestParam::ProxyConnectTimeout => "proxy_connect_timeout",
            RequestParam::ProxyReadTimeout => "proxy_read_timeout",
            RequestParam::ProxySendTimeout => "proxy_send_timeout",
            RequestParam::ProxyBuffering => "proxy_buffering",
        }
    }

    /// Which family of values this parameter takes.
    pub fn kind(self) -> ParamKind {
        match self {
            RequestParam::ProxyBuffering => ParamKind::Toggle,
            RequestParam::ClientMaxBodySize => ParamKind::Size,
            _ => ParamKind::Duration,
        }
    }

    /// Whether `0` ("unlimited") is a legal value for this parameter.
    ///
    /// `proxy_buffering` only accepts `on`/`off`, so its form row has two
    /// states rather than three.
    pub fn supports_unlimited(self) -> bool {
        self.kind() != ParamKind::Toggle
    }

    /// How a value that failed validation is described to the caller.
    fn expectation(self) -> &'static str {
        match self.kind() {
            ParamKind::Size => "must be a size with one of the units k, m or g (for example 100m)",
            ParamKind::Duration => {
                "must be a duration with one of the units ms, s, m or h (for example 60s)"
            }
            ParamKind::Toggle => "must be on or off",
        }
    }
}

/// The value family of a request parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    /// `100m`, `2g`: a number with a size unit.
    Size,
    /// `60s`, `300ms`: a number with a time unit.
    Duration,
    /// `on` / `off`.
    Toggle,
}

/// The per-rule values as stored on `proxy_rules`.
///
/// `None` means "inherit the global default" and is the state every column
/// starts in for rules that predate this feature.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestParams {
    #[serde(default)]
    pub client_max_body_size: Option<String>,
    #[serde(default)]
    pub client_body_timeout: Option<String>,
    #[serde(default)]
    pub proxy_connect_timeout: Option<String>,
    #[serde(default)]
    pub proxy_read_timeout: Option<String>,
    #[serde(default)]
    pub proxy_send_timeout: Option<String>,
    #[serde(default)]
    pub proxy_buffering: Option<String>,
}

impl RequestParams {
    /// The stored value for `param`, if the rule overrides it.
    pub fn get(&self, param: RequestParam) -> Option<&str> {
        match param {
            RequestParam::ClientMaxBodySize => self.client_max_body_size.as_deref(),
            RequestParam::ClientBodyTimeout => self.client_body_timeout.as_deref(),
            RequestParam::ProxyConnectTimeout => self.proxy_connect_timeout.as_deref(),
            RequestParam::ProxyReadTimeout => self.proxy_read_timeout.as_deref(),
            RequestParam::ProxySendTimeout => self.proxy_send_timeout.as_deref(),
            RequestParam::ProxyBuffering => self.proxy_buffering.as_deref(),
        }
    }

    /// Every parameter overridden with the given values. This is what applying a
    /// preset to a rule means: six explicit values that stay individually
    /// editable afterwards.
    #[allow(dead_code)] // exercised by the generation tests; the form applies presets itself
    pub fn from_defaults(values: &RequestDefaults) -> Self {
        Self {
            client_max_body_size: Some(values.client_max_body_size.clone()),
            client_body_timeout: Some(values.client_body_timeout.clone()),
            proxy_connect_timeout: Some(values.proxy_connect_timeout.clone()),
            proxy_read_timeout: Some(values.proxy_read_timeout.clone()),
            proxy_send_timeout: Some(values.proxy_send_timeout.clone()),
            proxy_buffering: Some(values.proxy_buffering.clone()),
        }
    }
}

/// The global value every rule inherits for a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestDefaults {
    pub client_max_body_size: String,
    pub client_body_timeout: String,
    pub proxy_connect_timeout: String,
    pub proxy_read_timeout: String,
    pub proxy_send_timeout: String,
    pub proxy_buffering: String,
}

impl RequestDefaults {
    /// Defaults equal to the behaviour the generated config had before rules
    /// could override these parameters. Ticket #24 replaces the *source* of
    /// these values (per-rule inheritance comes from `app_settings`), not the
    /// values themselves.
    pub fn builtin() -> Self {
        Self {
            client_max_body_size: "100m".to_string(),
            client_body_timeout: "60s".to_string(),
            proxy_connect_timeout: "60s".to_string(),
            proxy_read_timeout: "60s".to_string(),
            proxy_send_timeout: "60s".to_string(),
            proxy_buffering: "on".to_string(),
        }
    }

    /// The default value for `param`.
    pub fn get(&self, param: RequestParam) -> &str {
        match param {
            RequestParam::ClientMaxBodySize => &self.client_max_body_size,
            RequestParam::ClientBodyTimeout => &self.client_body_timeout,
            RequestParam::ProxyConnectTimeout => &self.proxy_connect_timeout,
            RequestParam::ProxyReadTimeout => &self.proxy_read_timeout,
            RequestParam::ProxySendTimeout => &self.proxy_send_timeout,
            RequestParam::ProxyBuffering => &self.proxy_buffering,
        }
    }
}

/// The value a generated `location` must write for `param`: the rule's own
/// value when it has one, otherwise the global default. Inheritance is resolved
/// here so the generated config always spells out what actually runs.
pub fn effective<'a>(
    params: &'a RequestParams,
    defaults: &'a RequestDefaults,
    param: RequestParam,
) -> &'a str {
    params.get(param).unwrap_or_else(|| defaults.get(param))
}

/// One preset the form can apply in a single click.
#[derive(Debug, Clone, Serialize)]
pub struct RequestPreset {
    /// Stable id; the form maps it to its own translated label.
    pub id: &'static str,
    pub values: RequestDefaults,
}

/// The three shipped presets, in the order the dropdown lists them.
///
/// Values are in [`RequestParam::ALL`] order: body size, body-read timeout,
/// connect timeout, read timeout, send timeout, buffering.
pub fn presets() -> Vec<RequestPreset> {
    let preset = |id: &'static str, values: [&str; 6]| RequestPreset {
        id,
        values: RequestDefaults {
            client_max_body_size: values[0].to_string(),
            client_body_timeout: values[1].to_string(),
            proxy_connect_timeout: values[2].to_string(),
            proxy_read_timeout: values[3].to_string(),
            proxy_send_timeout: values[4].to_string(),
            proxy_buffering: values[5].to_string(),
        },
    };

    vec![
        preset("large_upload", ["2g", "300s", "10s", "300s", "300s", "on"]),
        preset(
            "ai_streaming",
            ["100m", "60s", "10s", "3600s", "3600s", "off"],
        ),
        preset("long_tasks", ["100m", "60s", "10s", "3600s", "3600s", "on"]),
    ]
}

/// What the rule form needs to render one request-parameter row.
#[derive(Debug, Clone, Serialize)]
pub struct RequestParamInfo {
    /// The nginx directive, also the key of the stored value and the form field.
    pub directive: &'static str,
    /// Which unit rules the custom input follows.
    pub kind: ParamKind,
    /// Whether the row offers the third ("unlimited") state.
    pub supports_unlimited: bool,
}

/// What the rule form needs to render the request-parameter section.
#[derive(Debug, Clone, Serialize)]
pub struct RequestParamCatalog {
    /// Values an inheriting row shows.
    pub defaults: RequestDefaults,
    /// The six rows, in display order.
    pub params: Vec<RequestParamInfo>,
    pub presets: Vec<RequestPreset>,
}

/// Read the global defaults from the single place that owns them.
///
/// Today that is the built-in set. Ticket #24 swaps the body to read
/// `app_settings` under `defaults.request.*`; callers keep using this function.
pub fn load_defaults(_conn: &Connection) -> Result<RequestDefaults, AppError> {
    Ok(RequestDefaults::builtin())
}

/// The defaults, row metadata and presets the form renders.
pub fn catalog(conn: &Connection) -> Result<RequestParamCatalog, AppError> {
    Ok(RequestParamCatalog {
        defaults: load_defaults(conn)?,
        params: RequestParam::ALL
            .iter()
            .map(|param| RequestParamInfo {
                directive: param.directive(),
                kind: param.kind(),
                supports_unlimited: param.supports_unlimited(),
            })
            .collect(),
        presets: presets(),
    })
}

/// Check one stored value. `None` (inherit) never reaches this function.
pub fn validate(param: RequestParam, value: &str) -> Result<(), AppError> {
    let value = value.trim();
    if is_valid(param.kind(), value) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "Request parameter '{}' {}; got '{}'",
        param.directive(),
        param.expectation(),
        value
    )))
}

/// Check the values a create/update payload would store. `None` is inherit and
/// needs no check.
pub fn validate_all(values: [(RequestParam, Option<&str>); 6]) -> Result<(), AppError> {
    for (param, value) in values {
        if let Some(value) = value {
            validate(param, value)?;
        }
    }
    Ok(())
}

fn is_valid(kind: ParamKind, value: &str) -> bool {
    match kind {
        ParamKind::Size => is_size(value),
        ParamKind::Duration => is_duration(value),
        ParamKind::Toggle => matches!(value, "on" | "off"),
    }
}

/// `0` is the stored "unlimited" value. It is produced by the unlimited state,
/// not typed into the custom input: the form rejects a typed bare `0` and points
/// at the state instead.
fn is_size(value: &str) -> bool {
    if value == "0" {
        return true;
    }
    let Some((number, unit)) = split_unit(value) else {
        return false;
    };
    !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(unit.to_ascii_lowercase().as_str(), "k" | "m" | "g")
}

fn is_duration(value: &str) -> bool {
    if value == "0" {
        return true;
    }
    // Longest suffix first: `1ms` must not be read as `1m` + stray `s`.
    for unit in ["ms", "s", "m", "h"] {
        if let Some(number) = value.strip_suffix(unit) {
            return !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit());
        }
    }
    false
}

/// Split `100m` into `("100", "m")`. `None` when the value does not end in an
/// ASCII letter, so `100` stays a bare number.
fn split_unit(value: &str) -> Option<(&str, &str)> {
    let unit = value.chars().next_back()?;
    if !unit.is_ascii_alphabetic() {
        return None;
    }
    let split = value.len() - unit.len_utf8();
    Some((&value[..split], &value[split..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_are_the_nginx_names_in_form_order() {
        assert_eq!(
            RequestParam::ALL.map(RequestParam::directive),
            [
                "client_max_body_size",
                "client_body_timeout",
                "proxy_connect_timeout",
                "proxy_read_timeout",
                "proxy_send_timeout",
                "proxy_buffering",
            ]
        );
    }

    #[test]
    fn built_in_defaults_match_the_behaviour_the_config_had_before_overrides() {
        let defaults = RequestDefaults::builtin();
        assert_eq!(defaults.client_max_body_size, "100m");
        assert_eq!(defaults.client_body_timeout, "60s");
        assert_eq!(defaults.proxy_connect_timeout, "60s");
        assert_eq!(defaults.proxy_read_timeout, "60s");
        assert_eq!(defaults.proxy_send_timeout, "60s");
        assert_eq!(defaults.proxy_buffering, "on");
    }

    #[test]
    fn sizes_need_one_of_the_supported_units() {
        for valid in ["1k", "100m", "2g", "100M", "2G", "0"] {
            assert!(
                validate(RequestParam::ClientMaxBodySize, valid).is_ok(),
                "`{valid}` must be accepted as a size"
            );
        }
        for invalid in ["100", "1t", "", "m", "-1m", "1.5g", "1 m", "1mb"] {
            assert!(
                validate(RequestParam::ClientMaxBodySize, invalid).is_err(),
                "`{invalid}` must be rejected as a size"
            );
        }
    }

    #[test]
    fn durations_accept_ms_s_m_h_and_reject_bare_numbers() {
        for valid in ["300ms", "60s", "5m", "1h", "0"] {
            assert!(
                validate(RequestParam::ProxyReadTimeout, valid).is_ok(),
                "`{valid}` must be accepted as a duration"
            );
        }
        for invalid in ["30", "30sx", "1d", "", "s", "-5s", "1.5s"] {
            assert!(
                validate(RequestParam::ProxyReadTimeout, invalid).is_err(),
                "`{invalid}` must be rejected as a duration"
            );
        }
    }

    #[test]
    fn a_bare_number_names_the_directive_and_the_expected_units() {
        let message = validate(RequestParam::ProxyReadTimeout, "30")
            .unwrap_err()
            .to_string();
        assert!(message.contains("proxy_read_timeout"), "{message}");
        assert!(message.contains("ms, s, m or h"), "{message}");
        assert!(message.contains("'30'"), "{message}");
    }

    #[test]
    fn buffering_only_accepts_on_or_off_and_offers_no_unlimited_state() {
        assert!(validate(RequestParam::ProxyBuffering, "on").is_ok());
        assert!(validate(RequestParam::ProxyBuffering, "off").is_ok());
        assert!(validate(RequestParam::ProxyBuffering, "0").is_err());
        assert!(validate(RequestParam::ProxyBuffering, "true").is_err());

        assert!(!RequestParam::ProxyBuffering.supports_unlimited());
        for param in RequestParam::ALL {
            if param != RequestParam::ProxyBuffering {
                assert!(param.supports_unlimited(), "{}", param.directive());
            }
        }
    }

    #[test]
    fn every_preset_value_passes_the_same_validation_as_a_typed_value() {
        for preset in presets() {
            for param in RequestParam::ALL {
                let value = preset.values.get(param);
                validate(param, value).unwrap_or_else(|error| {
                    panic!("preset `{}` has an invalid value: {}", preset.id, error)
                });
            }
        }
    }

    #[test]
    fn presets_match_the_three_shipped_profiles() {
        let presets = presets();
        let values = |id: &str| {
            presets
                .iter()
                .find(|preset| preset.id == id)
                .unwrap_or_else(|| panic!("missing preset `{id}`"))
                .values
                .clone()
        };

        let large = values("large_upload");
        assert_eq!(
            [
                large.client_max_body_size.as_str(),
                large.client_body_timeout.as_str(),
                large.proxy_connect_timeout.as_str(),
                large.proxy_read_timeout.as_str(),
                large.proxy_send_timeout.as_str(),
                large.proxy_buffering.as_str(),
            ],
            ["2g", "300s", "10s", "300s", "300s", "on"]
        );

        let streaming = values("ai_streaming");
        assert_eq!(
            [
                streaming.client_max_body_size.as_str(),
                streaming.client_body_timeout.as_str(),
                streaming.proxy_connect_timeout.as_str(),
                streaming.proxy_read_timeout.as_str(),
                streaming.proxy_send_timeout.as_str(),
                streaming.proxy_buffering.as_str(),
            ],
            ["100m", "60s", "10s", "3600s", "3600s", "off"]
        );

        let long_tasks = values("long_tasks");
        assert_eq!(
            [
                long_tasks.client_max_body_size.as_str(),
                long_tasks.client_body_timeout.as_str(),
                long_tasks.proxy_connect_timeout.as_str(),
                long_tasks.proxy_read_timeout.as_str(),
                long_tasks.proxy_send_timeout.as_str(),
                long_tasks.proxy_buffering.as_str(),
            ],
            ["100m", "60s", "10s", "3600s", "3600s", "on"]
        );
    }

    #[test]
    fn applying_a_preset_overrides_all_six_and_each_stays_editable() {
        let preset = presets().into_iter().nth(1).expect("AI streaming preset");
        let mut params = RequestParams::from_defaults(&preset.values);
        for param in RequestParam::ALL {
            assert_eq!(
                params.get(param),
                Some(preset.values.get(param)),
                "applying a preset must fill `{}`",
                param.directive()
            );
        }

        params.proxy_buffering = Some("on".to_string());
        assert_eq!(params.get(RequestParam::ProxyBuffering), Some("on"));
    }

    #[test]
    fn a_stored_value_wins_and_a_missing_one_falls_back_to_the_default() {
        let defaults = RequestDefaults::builtin();
        let mut params = RequestParams::default();

        for param in RequestParam::ALL {
            assert_eq!(effective(&params, &defaults, param), defaults.get(param));
        }

        params.client_max_body_size = Some("2g".to_string());
        params.proxy_read_timeout = Some("0".to_string());
        assert_eq!(
            effective(&params, &defaults, RequestParam::ClientMaxBodySize),
            "2g"
        );
        assert_eq!(
            effective(&params, &defaults, RequestParam::ProxyReadTimeout),
            "0"
        );
        assert_eq!(
            effective(&params, &defaults, RequestParam::ClientBodyTimeout),
            "60s"
        );
    }

    #[test]
    fn the_catalog_serialises_the_contract_the_form_reads() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let catalog = catalog(&conn).unwrap();
        let json = serde_json::to_value(&catalog).unwrap();

        assert_eq!(json["defaults"]["client_max_body_size"], "100m");
        assert_eq!(json["defaults"]["proxy_buffering"], "on");
        assert_eq!(json["params"].as_array().unwrap().len(), 6);
        assert_eq!(json["params"][0]["directive"], "client_max_body_size");
        assert_eq!(json["params"][0]["kind"], "size");
        assert_eq!(json["params"][0]["supports_unlimited"], true);
        assert_eq!(json["params"][1]["kind"], "duration");
        assert_eq!(json["params"][5]["directive"], "proxy_buffering");
        assert_eq!(json["params"][5]["kind"], "toggle");
        assert_eq!(json["params"][5]["supports_unlimited"], false);
        assert_eq!(json["presets"][0]["id"], "large_upload");
        assert_eq!(json["presets"][0]["values"]["proxy_read_timeout"], "300s");
        assert_eq!(json["presets"][1]["id"], "ai_streaming");
        assert_eq!(json["presets"][2]["id"], "long_tasks");
    }
}
