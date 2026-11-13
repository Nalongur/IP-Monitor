//! Detection pipeline (spec §5, §7.2, §8.3).
//!
//! One run builds a client from the selected mode, then asks ipleak.net twice —
//! once over IPv4 and once over IPv6 — and reports each result independently.
//! A failure on one family never invents data for the other.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::proxy::{DetectionMode, ProxyResolver, ResolvedProxy};

pub const IPV4_ENDPOINT: &str = "https://ipv4.ipleak.net/json/";
pub const IPV6_ENDPOINT: &str = "https://ipv6.ipleak.net/json/";

/// Spec §8.3: eight seconds per request, one retry only for transient errors.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// Outcome of one family's lookup (spec §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldStatus {
    Success,
    /// Reachable network stack, but no route/address for this family.
    Unavailable,
    Timeout,
    Error,
}

impl FieldStatus {
    fn from_error(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            FieldStatus::Timeout
        } else if error.is_connect() {
            // No IPv6 route, DNS failure, refused connection: the family is not
            // usable right now rather than the service being wrong.
            FieldStatus::Unavailable
        } else {
            FieldStatus::Error
        }
    }

    /// The innermost cause, because reqwest's headline is always the same
    /// "error sending request for url (…)" and says nothing about *why*.
    fn reason(error: &reqwest::Error, proxy_used: bool) -> String {
        // Arbitrary nested proxy/TLS errors can echo a credential-bearing URL.
        // Keep a useful category without copying untrusted error strings.
        if proxy_used {
            return if error.is_timeout() { "proxy request timed out" }
                else if error.is_connect() { "proxy connection failed" }
                else { "proxy request failed" }.into();
        }
        let mut source: &dyn std::error::Error = error;
        let mut deepest = error.to_string();
        while let Some(next) = source.source() {
            deepest = next.to_string();
            source = next;
        }
        deepest
    }

    /// Transient failures are worth one retry; 4xx/TLS/auth are not (spec §8.3).
    fn is_transient(self) -> bool {
        matches!(self, FieldStatus::Timeout | FieldStatus::Unavailable)
    }
}

/// Normalized ipleak payload. Every field is optional: a missing key must not
/// fail the whole parse (spec §5.2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpInfo {
    pub ip: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub city_name: Option<String>,
    pub isp_name: Option<String>,
    pub as_number: Option<u64>,
    pub as_network: Option<String>,
    pub time_zone: Option<String>,
}

/// One completed detection attempt (spec §7.2).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub mode: DetectionMode,
    pub ipv4: Option<IpInfo>,
    pub ipv4_status: FieldStatus,
    pub ipv6: Option<String>,
    pub ipv6_status: FieldStatus,
    /// Same lookup as `ipv6`, kept as a full record so the card can show the
    /// region/ISP of the v6 exit as well (the contract's `ipv6` stays a string).
    pub ipv6_info: Option<IpInfo>,
    /// RFC 3339, UTC.
    pub checked_at: String,
    pub duration_ms: u64,
    /// Where the request went, for the "is this really my mode" question.
    /// Never contains credentials (spec §6.3).
    pub route: String,
    /// Human-readable failure reason; `None` when the run produced an address.
    pub detail: Option<String>,
}

/// What a run can fail with before any request is even attempted.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionError {
    pub kind: String,
    pub message: String,
}

impl DetectionError {
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

/// Only the proxy origin is safe to expose. Userinfo, path and query remain
/// inside the networking layer and are never included in diagnostics.
fn proxy_route(proxy: &ResolvedProxy) -> String {
    match proxy {
        ResolvedProxy::None => "direct".into(),
        ResolvedProxy::Url { url, source, .. } => {
            let Ok(parsed) = reqwest::Url::parse(url) else {
                return format!("{source} [invalid proxy]");
            };
            let Some(host) = parsed.host() else {
                return format!("{source} [invalid proxy]");
            };
            let port = parsed.port().map(|port| format!(":{port}")).unwrap_or_default();
            format!("{source} {}://{host}{port}", parsed.scheme())
        }
    }
}

/// Builds the client for one run. `None` means "direct" (spec §6.1).
fn build_client(proxy: &ResolvedProxy) -> Result<reqwest::Client, DetectionError> {
    let mut builder = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("IP-Monitor/", env!("CARGO_PKG_VERSION"), " (+tauri)"));

    builder = match proxy {
        // `no_proxy()` also disables the automatic discovery reqwest would
        // otherwise inherit from the environment (spec §6.1).
        ResolvedProxy::None => builder.no_proxy(),
        ResolvedProxy::Url {
            url, credentials, ..
        } => {
            let mut p = reqwest::Proxy::all(url.as_str()).map_err(|_| {
                DetectionError::new("proxy", "cannot use the configured proxy URL")
            })?;
            if let Some((user, pass)) = credentials {
                p = p.basic_auth(user, pass);
            }
            builder.proxy(p)
        }
    };

    builder
        .build()
        .map_err(|_| DetectionError::new("client", "cannot create network client"))
}

/// The seam other providers can plug into (spec §5.5).
pub trait IpInfoProvider: Send + Sync {
    fn endpoint(&self) -> &'static str;
}

pub struct IpLeak;

impl IpInfoProvider for IpLeak {
    fn endpoint(&self) -> &'static str {
        IPV4_ENDPOINT
    }
}

/// Parses one ipleak response body.
///
/// Deliberately tolerant: the API is third-party, so unknown keys are ignored
/// and wrong scalar types are coerced rather than rejected (spec §5.2).
pub fn parse_ip_info(body: &str) -> Result<IpInfo, DetectionError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| DetectionError::new("parse", error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| DetectionError::new("parse", "response is not a JSON object"))?;

    let string = |key: &str| -> Option<String> {
        object
            .get(key)
            .and_then(|value| match value {
                serde_json::Value::String(text) => Some(text.trim().to_string()),
                serde_json::Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
    };

    let country_code = string("country_code").map(|code| code.to_ascii_uppercase());

    Ok(IpInfo {
        ip: string("ip"),
        country_code,
        country_name: string("country_name"),
        city_name: string("city_name"),
        isp_name: string("isp_name"),
        // `as_number` arrives as a number on some responses and as "12345" or
        // "AS12345" on others.
        as_number: object.get("as_number").and_then(|value| match value {
            serde_json::Value::Number(number) => number.as_u64(),
            serde_json::Value::String(text) => text
                .trim()
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .parse::<u64>()
                .ok(),
            _ => None,
        }),
        as_network: string("as_network"),
        time_zone: string("time_zone"),
    })
}

/// One family lookup with the retry policy from spec §8.3.
///
/// The message is kept alongside the status: "Error" alone tells the user
/// nothing about whether the proxy refused the connection or the service is
/// down, and spec §6.3 requires the failure reason to be visible.
fn normalize_address(address: Option<&str>, ipv4: bool) -> Option<String> {
    address?
        .parse::<std::net::IpAddr>()
        .ok()
        .filter(|ip| ip.is_ipv4() == ipv4)
        .map(|ip| ip.to_string())
}

async fn lookup(client: &reqwest::Client, url: &'static str, proxy_used: bool) -> (Option<IpInfo>, FieldStatus, Option<String>) {
    let mut last = FieldStatus::Error;
    let mut message: Option<String> = None;

    for attempt in 0..2 {
        let outcome = match client.get(url).send().await {
            Ok(response) if response.status().is_redirection() => {
                (None, FieldStatus::Error, Some("detection endpoint returned a redirect".into()))
            }
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.text().await {
                    Ok(body) => match parse_ip_info(&body) {
                        Ok(mut info) => {
                            let ip = normalize_address(info.ip.as_deref(), url == IPV4_ENDPOINT);
                            if let Some(ip) = ip {
                                info.ip = Some(ip.to_string());
                                return (Some(info), FieldStatus::Success, None);
                            }
                            (None, FieldStatus::Error, Some("response had no valid address for this IP family".into()))
                        }
                        Err(error) => (None, FieldStatus::Error, Some(format!("unreadable response: {}", error.message))),
                    },
                    Err(error) => (None, FieldStatus::from_error(&error), Some(FieldStatus::reason(&error, proxy_used))),
                },
                Err(error) => (None, FieldStatus::from_error(&error), Some(FieldStatus::reason(&error, proxy_used))),
            },
            Err(error) => (None, FieldStatus::from_error(&error), Some(FieldStatus::reason(&error, proxy_used))),
        };

        last = outcome.1;
        message = outcome.2.clone();
        if !last.is_transient() || attempt == 1 {
            return outcome;
        }
    }

    (None, last, message)
}

/// Runs one detection for `mode`.
pub async fn detect(mode: DetectionMode, custom: Option<&crate::proxy::CustomProxyConfig>) -> Result<DetectionResult, DetectionError> {
    let started = Instant::now();
    let checked_at = iso8601_now();

    let resolved = match mode {
        DetectionMode::Direct => crate::proxy::DirectResolver.resolve(),
        DetectionMode::SystemProxy => crate::proxy::SystemProxyResolver.resolve(),
        DetectionMode::CustomProxy => {
            let config = custom.ok_or_else(|| {
                DetectionError::new("proxy", "no custom proxy is configured yet")
            })?;
            crate::proxy::CustomProxyResolver(config).resolve()
        }
    }
    .map_err(|error| DetectionError::new("proxy", error.to_string()))?;

    let route = proxy_route(&resolved);
    let proxy_used = !matches!(resolved, ResolvedProxy::None);

    let client = build_client(&resolved)?;

    // Both families go out together so a missing IPv6 route cannot add eight
    // seconds to the IPv4 answer.
    let v4 = tauri::async_runtime::spawn({
        let client = client.clone();
        async move { lookup(&client, IPV4_ENDPOINT, proxy_used).await }
    });
    let v6 = tauri::async_runtime::spawn({
        let client = client.clone();
        async move { lookup(&client, IPV6_ENDPOINT, proxy_used).await }
    });

    let (ipv4, ipv4_status, ipv4_error) = v4
        .await
        .map_err(|error| DetectionError::new("runtime", error.to_string()))?;
    let (ipv6_info, ipv6_status, ipv6_error) = v6
        .await
        .map_err(|error| DetectionError::new("runtime", error.to_string()))?;

    // One line the card can show when the run failed, so the user learns *why*
    // instead of just reading "Error" (spec §6.3).
    let detail = if ipv4_status == FieldStatus::Success {
        ipv6_error.map(|reason| format!("{route} — {reason}"))
    } else {
        ipv4_error.map(|reason| format!("{route} — {reason}"))
    };

    Ok(DetectionResult {
        mode,
        ipv4,
        ipv4_status,
        ipv6: ipv6_info.as_ref().and_then(|info| info.ip.clone()),
        ipv6_status,
        ipv6_info,
        checked_at,
        duration_ms: started.elapsed().as_millis() as u64,
        route,
        detail,
    })
}

/// RFC 3339 UTC without pulling in a date crate.
fn iso8601_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

/// Howard Hinnant's days-to-civil conversion.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_addresses_of_the_requested_family_are_accepted() {
        assert_eq!(normalize_address(Some("192.0.2.42"), true).as_deref(), Some("192.0.2.42"));
        assert_eq!(normalize_address(Some("2001:0DB8:0:0::42"), false).as_deref(), Some("2001:db8::42"));
        assert!(normalize_address(Some("192.0.2.42"), false).is_none());
        assert!(normalize_address(Some("2001:db8::42"), true).is_none());
        assert!(normalize_address(Some("unknown"), true).is_none());
    }

    #[test]
    fn proxy_diagnostics_never_expose_url_userinfo() {
        let proxy = ResolvedProxy::Url {
            url: "http://synthetic-user:synthetic-pass@127.0.0.1:18080".into(),
            credentials: None,
            source: "windows",
        };
        assert_eq!(proxy_route(&proxy), "windows http://127.0.0.1:18080");
        let bad = ResolvedProxy::Url {
            url: "http://synthetic-user:synthetic-pass@[invalid".into(),
            credentials: None,
            source: "windows",
        };
        assert!(!proxy_route(&bad).contains("synthetic"));
        let error = build_client(&bad).unwrap_err().message;
        assert!(!error.contains("synthetic"));
    }

    #[test]
    fn refused_authenticated_proxy_does_not_disclose_credentials() {
        let proxy = ResolvedProxy::Url {
            url: "http://synthetic-user:synthetic-pass@127.0.0.1:1".into(),
            credentials: None,
            source: "windows",
        };
        let client = build_client(&proxy).unwrap();
        let (_, status, reason) = tauri::async_runtime::block_on(lookup(&client, IPV4_ENDPOINT, true));
        assert_ne!(status, FieldStatus::Success);
        let detail = reason.unwrap_or_default();
        assert!(!detail.contains("synthetic-user"));
        assert!(!detail.contains("synthetic-pass"));
        assert!(!detail.is_empty());
    }

    #[test]
    fn parses_a_full_payload() {
        let body = r#"{
            "ip": "203.0.113.7",
            "country_code": "jp",
            "country_name": "Japan",
            "city_name": "Tokyo",
            "isp_name": "Example",
            "as_number": 64500,
            "as_network": "203.0.113.0/24",
            "time_zone": "Asia/Tokyo"
        }"#;
        let info = parse_ip_info(body).unwrap();
        assert_eq!(info.ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(info.country_code.as_deref(), Some("JP"));
        assert_eq!(info.as_number, Some(64500));
    }

    #[test]
    fn missing_fields_never_fail_the_parse() {
        let info = parse_ip_info(r#"{"ip":"2001:db8::1"}"#).unwrap();
        assert_eq!(info.ip.as_deref(), Some("2001:db8::1"));
        assert!(info.country_name.is_none());
        assert!(info.as_number.is_none());
    }

    #[test]
    fn accepts_as_number_as_text() {
        let info = parse_ip_info(r#"{"ip":"1.1.1.1","as_number":"AS13335"}"#).unwrap();
        assert_eq!(info.as_number, Some(13335));
    }

    #[test]
    fn rejects_non_object_bodies() {
        assert!(parse_ip_info("[1,2,3]").is_err());
        assert!(parse_ip_info("not json").is_err());
    }

    #[test]
    fn iso8601_is_well_formed() {
        let stamp = iso8601_now();
        assert_eq!(stamp.len(), 20);
        assert!(stamp.ends_with('Z'));
        assert_eq!(&stamp[4..5], "-");
    }
}
