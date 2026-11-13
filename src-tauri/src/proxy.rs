//! Network modes and proxy resolution (spec §6).
//!
//! The whole point of this module is that the mode shown in the UI is the mode
//! the request actually used. A client is therefore built *per detection run*
//! from a resolver that either returns a concrete proxy or an error — never a
//! silent fallback to a direct connection (spec §6.2, §6.3).

use serde::{Deserialize, Serialize};

/// How the outbound request reaches ipleak.net (spec §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMode {
    Direct,
    SystemProxy,
    CustomProxy,
}

/// Transport of a user-supplied proxy (spec §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Http,
    Https,
    Socks5,
}

impl ProxyProtocol {
    /// URL scheme reqwest understands for this transport.
    fn scheme(self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5h",
        }
    }
}

/// What the user typed into the custom-proxy form.
///
/// The password is *not* persisted by the settings file (spec §10.2); it only
/// lives in this struct for the lifetime of the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// Never serialized back out: the frontend only ever sets it.
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

impl CustomProxyConfig {
    /// Validates host/port and normalizes the host (spec §6.3: the address has
    /// to be checked before it is used, not after the request fails).
    pub fn validate(&self) -> Result<(), ProxyError> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err(ProxyError::Invalid("host is empty".into()));
        }
        // Hostname or IP literal only: no scheme, no path, no userinfo. A
        // pasted "http://user:pass@host:8080" must be rejected rather than
        // half-understood.
        if host.chars().any(|c| c.is_whitespace() || c.is_control() || "/\\@?#%".contains(c)) {
            return Err(ProxyError::Invalid(
                "host must be a name or an IP address, without scheme or credentials".into(),
            ));
        }
        if self.port == 0 {
            return Err(ProxyError::Invalid("port must be 1-65535".into()));
        }
        // A colon is legal in an IPv6 literal and illegal anywhere else, so the
        // only thing that may contain one is a literal that parses as IPv6.
        // "127.0.0.1:8080" pasted into the host field is a port, not a host.
        let bare = host.strip_prefix('[').and_then(|host| host.strip_suffix(']')).unwrap_or(host);
        let valid_host = if host.contains([':', '[', ']']) {
            bare.parse::<std::net::Ipv6Addr>().is_ok()
        } else {
            reqwest::Url::parse(&format!("http://{host}"))
                .is_ok_and(|url| url.has_host() && url.port().is_none())
        };
        if !valid_host {
            return Err(ProxyError::Invalid(
                "host must not include a port; fill the port field instead".into(),
            ));
        }
        if self.username.is_some() != self.password.is_some() {
            return Err(ProxyError::Invalid(
                "username and password must be provided together".into(),
            ));
        }
        Ok(())
    }

    /// `host:port` without credentials, safe to log.
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.host.trim(), self.port)
    }

    fn to_url(&self) -> Result<String, ProxyError> {
        self.validate()?;
        // IPv6 literals need brackets inside a URL.
        let host = self.host.trim();
        if host.contains(':') && !host.starts_with('[') {
            Ok(format!("{}://[{}]:{}", self.protocol.scheme(), host, self.port))
        } else {
            Ok(format!("{}://{}:{}", self.protocol.scheme(), host, self.port))
        }
    }
}

/// A proxy that a client can actually be built from.
#[derive(Debug, Clone)]
pub enum ResolvedProxy {
    /// Direct connection: no proxy at all (spec §6.1).
    None,
    /// A proxy URL, optionally with credentials that must not be logged.
    Url {
        url: String,
        credentials: Option<(String, String)>,
        /// Human-readable origin of the setting, for the diagnostics line.
        source: &'static str,
    },
}

impl ResolvedProxy {
    /// The endpoint described by this resolution, without any credentials.
    #[cfg(test)]
    pub fn url(&self) -> &str {
        match self {
            ResolvedProxy::None => "",
            ResolvedProxy::Url { url, .. } => url,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProxyError {
    #[error("Windows has no proxy configured")]
    NotConfigured,
    #[error(
        "Windows is set to use an automatic proxy script (PAC/WPAD); this build cannot \
         follow it and will not guess a proxy instead"
    )]
    UnsupportedAutoConfig,
    #[error("invalid custom proxy: {0}")]
    Invalid(String),
    #[error("cannot read the Windows proxy settings ({0})")]
    System(String),
}

/// Turns a mode into something a client can be built from (spec §6.4).
pub trait ProxyResolver {
    fn resolve(&self) -> Result<ResolvedProxy, ProxyError>;
}

/// Direct: deliberately nothing to resolve.
pub struct DirectResolver;

impl ProxyResolver for DirectResolver {
    fn resolve(&self) -> Result<ResolvedProxy, ProxyError> {
        Ok(ResolvedProxy::None)
    }
}

pub struct CustomProxyResolver<'a>(pub &'a CustomProxyConfig);

impl ProxyResolver for CustomProxyResolver<'_> {
    fn resolve(&self) -> Result<ResolvedProxy, ProxyError> {
        let url = self.0.to_url()?;
        let credentials = match (&self.0.username, &self.0.password) {
            (Some(user), Some(pass)) => Some((user.clone(), pass.clone())),
            _ => None,
        };
        Ok(ResolvedProxy::Url {
            url,
            credentials,
            source: "custom",
        })
    }
}

#[cfg(target_os = "windows")]
pub struct SystemProxyResolver;

#[cfg(target_os = "windows")]
impl ProxyResolver for SystemProxyResolver {
    fn resolve(&self) -> Result<ResolvedProxy, ProxyError> {
        windows_impl::resolve_system_proxy()
    }
}

#[cfg(not(target_os = "windows"))]
pub struct SystemProxyResolver;

#[cfg(not(target_os = "windows"))]
impl ProxyResolver for SystemProxyResolver {
    fn resolve(&self) -> Result<ResolvedProxy, ProxyError> {
        Err(ProxyError::System(
            "reading the system proxy is only implemented on Windows".into(),
        ))
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::{ProxyError, ResolvedProxy};
    use std::ptr::null_mut;
    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpGetIEProxyConfigForCurrentUser, WINHTTP_CURRENT_USER_IE_PROXY_CONFIG,
    };

    /// Reads the per-user proxy configuration through WinHTTP (spec §6.2).
    ///
    /// Deliberately *not* `HTTP_PROXY`/`HTTPS_PROXY`: those are not what
    /// Windows uses for applications, and treating them as the system proxy is
    /// exactly the confusion the spec warns about.
    pub fn resolve_system_proxy() -> Result<ResolvedProxy, ProxyError> {
        // SAFETY: the struct is plain-old-data; WinHTTP fills it and hands back
        // strings that we free ourselves with the global allocator.
        unsafe {
            let mut config: WINHTTP_CURRENT_USER_IE_PROXY_CONFIG = std::mem::zeroed();
            if WinHttpGetIEProxyConfigForCurrentUser(&mut config) == 0 {
                return Err(ProxyError::System(
                    std::io::Error::last_os_error().to_string(),
                ));
            }

            let auto_config = take_wide_string(config.lpszAutoConfigUrl);
            let proxy_server = take_wide_string(config.lpszProxy);
            // The configured mode intentionally routes every detection through
            // the proxy, but WinHTTP still transfers ownership of this string.
            drop(take_wide_string(config.lpszProxyBypass));
            let auto_detect = config.fAutoDetect != 0;

            // PAC/WPAD cannot be honoured, and silently going direct would lie
            // about the mode (spec §6.2 step 4).
            if auto_config.is_some() || auto_detect {
                return Err(ProxyError::UnsupportedAutoConfig);
            }

            let Some(proxy) = proxy_server else {
                return Err(ProxyError::NotConfigured);
            };
            parse_proxy_server(&proxy)
        }
    }

    /// Frees a WinHTTP-allocated UTF-16 string and returns it as a `String`.
    unsafe fn take_wide_string(raw: *mut u16) -> Option<String> {
        if raw.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(raw, len);
        let text = String::from_utf16_lossy(slice);
        // WinHTTP allocates with GlobalAlloc; freeing through the same allocator
        // is what the docs prescribe.
        windows_sys::Win32::Foundation::GlobalFree(raw as _);
        let trimmed = text.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    }

    /// Parses the `ProxyServer` value.
    ///
    /// It is either `host:port` or a per-scheme list such as
    /// `http=host:port;https=host:port;ftp=...` or `socks=host:port`. We take the
    /// HTTPS entry when present (it also covers our HTTPS requests), then HTTP,
    /// then SOCKS. A SOCKS entry keeps the SOCKS scheme: handing it to reqwest as
    /// an HTTP proxy would connect but never reach the target.
    pub(super) fn parse_proxy_server(value: &str) -> Result<ResolvedProxy, ProxyError> {
        let mut http = None;
        let mut https = None;
        let mut socks = None;
        let mut plain = None;

        for part in value.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.split_once('=') {
                Some((scheme, endpoint)) => {
                    let endpoint = endpoint.trim();
                    match scheme.trim().to_ascii_lowercase().as_str() {
                        "https" => https = Some(endpoint.to_string()),
                        "http" => http = Some(endpoint.to_string()),
                        "socks" | "socks4" | "socks5" | "socks5h" => {
                            socks = Some(endpoint.to_string())
                        }
                        _ => {}
                    }
                }
                None => plain = Some(part.to_string()),
            }
        }

        if let Some(endpoint) = socks {
            if https.is_none() && http.is_none() {
                // `socks5h` resolves the name at the proxy, which is what a user
                // configuring a SOCKS proxy expects; `socks5` would leak DNS.
                let url = if endpoint.contains("://") {
                    endpoint
                } else {
                    format!("socks5h://{endpoint}")
                };
                return Ok(ResolvedProxy::Url {
                    url,
                    credentials: None,
                    source: "windows",
                });
            }
        }

        let endpoint = https.or(http).or(plain).ok_or_else(|| {
            ProxyError::System("the Windows proxy list has no usable entry".into())
        })?;

        // A scheme-qualified entry ("http=host:port") is still a plain HTTP
        // proxy; anything we cannot address is reported, not guessed.
        if endpoint.contains("://") {
            return Ok(ResolvedProxy::Url {
                url: endpoint,
                credentials: None,
                source: "windows",
            });
        }
        Ok(ResolvedProxy::Url {
            url: format!("http://{endpoint}"),
            credentials: None,
            source: "windows",
        })
    }

    // Keeps the `null_mut` import used on builds where the FFI call is elided.
    #[allow(dead_code)]
    fn _unused() -> *mut u16 {
        null_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "windows")]
    use super::windows_impl::parse_proxy_server;

    fn cfg(host: &str, port: u16) -> CustomProxyConfig {
        CustomProxyConfig {
            protocol: ProxyProtocol::Socks5,
            host: host.into(),
            port,
            username: None,
            password: None,
        }
    }

    #[test]
    fn rejects_hosts_that_carry_credentials_or_scheme() {
        assert!(cfg("http://user:pass@127.0.0.1", 8080).validate().is_err());
        assert!(cfg("127.0.0.1:8080", 8080).validate().is_err());
        assert!(cfg("proxy.example?x", 8080).validate().is_err());
        assert!(cfg("proxy.example#x", 8080).validate().is_err());
        assert!(cfg("proxy.example\\x", 8080).validate().is_err());
        assert!(cfg("", 8080).validate().is_err());
        assert!(cfg("127.0.0.1", 0).validate().is_err());
    }

    #[test]
    fn accepts_plain_hosts_and_ipv6_literals() {
        assert!(cfg("127.0.0.1", 7890).validate().is_ok());
        assert!(cfg("proxy.local", 1080).validate().is_ok());
        let v6 = cfg("::1", 1080);
        assert!(v6.validate().is_ok());
        assert_eq!(v6.to_url().unwrap(), "socks5h://[::1]:1080");
    }

    #[test]
    fn custom_proxy_needs_both_or_neither_credential() {
        let mut c = cfg("127.0.0.1", 8080);
        c.username = Some("u".into());
        assert!(c.validate().is_err());
        c.password = Some("p".into());
        assert!(c.validate().is_ok());
    }

    #[test]
    fn system_proxy_list_prefers_https_then_http() {
        let both = parse_proxy_server("http=10.0.0.1:8080;https=10.0.0.2:8443").unwrap();
        assert_eq!(both.url(), "http://10.0.0.2:8443");
        let http_only = parse_proxy_server("http=10.0.0.1:8080").unwrap();
        assert_eq!(http_only.url(), "http://10.0.0.1:8080");
        let bare = parse_proxy_server("10.0.0.9:3128").unwrap();
        assert_eq!(bare.url(), "http://10.0.0.9:3128");
    }

    #[test]
    fn system_proxy_socks_entry_stays_socks() {
        let socks = parse_proxy_server("socks=127.0.0.1:7890").unwrap();
        assert_eq!(socks.url(), "socks5h://127.0.0.1:7890");
        // An HTTP entry wins when both are listed: that is what the machine
        // would use for HTTPS anyway.
        let mixed = parse_proxy_server("socks=127.0.0.1:7890;http=10.0.0.1:8080").unwrap();
        assert_eq!(mixed.url(), "http://10.0.0.1:8080");
    }

    #[test]
    fn system_proxy_without_a_usable_entry_is_an_error() {
        assert!(parse_proxy_server("ftp=10.0.0.1:21").is_err());
    }
}
