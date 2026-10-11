use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const MAX_HEADERS: usize = 32;
const MAX_HEADER_NAME_CHARS: usize = 64;
const MAX_HEADER_VALUE_CHARS: usize = 1024;
// framing, routing and our own marker belong to the proxy, and cookies to the app
const RESERVED_HEADERS: [&str; 11] = [
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "proxy-connection",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "x-server",
];

/// How bird's proxy treats one service's requests; the default changes nothing
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ProxySettings {
    /// Requests each visitor may make, counted per IP address (IPv6 per /64)
    pub rate_limit: Option<RateLimit>,
    /// Largest request body in MiB, 100 when unset
    pub max_body_mb: Option<BodyLimitMb>,
    /// Seconds the app has to start answering, 60 when unset
    pub timeout_secs: Option<ProxyTimeout>,
    /// Add HSTS on https, nosniff, frame and referrer policies unless the app sets them
    pub security_headers: bool,
    /// Response headers set on every answer, replacing the app's
    pub headers: ResponseHeaders,
    /// Answer 503 instead of passing requests to the app
    pub maintenance: bool,
    /// Take the visitor from `CF-Connecting-IP` on requests from Cloudflare's addresses, for
    /// rate limits and `X-Forwarded-For`
    pub cloudflare: bool,
}

impl ProxySettings {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct RateLimit {
    /// Sustained requests per second
    pub per_second: RatePerSecond,
    /// Requests allowed at once before the sustained rate applies
    pub burst: RateBurst,
}

macro_rules! bounded {
    ($(#[$doc:meta])* $name:ident, $min:literal, $max:literal, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[cfg_attr(feature = "openapi", schema(value_type = u32))]
        #[serde(try_from = "u32", into = "u32")]
        pub struct $name(u32);

        impl $name {
            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl TryFrom<u32> for $name {
            type Error = ValidationError;

            fn try_from(value: u32) -> Result<Self, Self::Error> {
                if ($min..=$max).contains(&value) {
                    Ok(Self(value))
                } else {
                    Err(ValidationError::Proxy(format!(
                        "{} must be {} to {}, not {value}",
                        $what, $min, $max
                    )))
                }
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

bounded!(
    /// 1 to 10000
    RatePerSecond, 1, 10_000, "requests per second"
);
bounded!(
    /// 1 to 100000
    RateBurst, 1, 100_000, "burst"
);
bounded!(
    /// 1 to 10240
    BodyLimitMb, 1, 10_240, "the body limit in MiB"
);
bounded!(
    /// 1 to 3600
    ProxyTimeout, 1, 3_600, "the timeout in seconds"
);

/// Header names are lowercase, values printable ASCII
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = BTreeMap<String, String>))]
#[serde(
    try_from = "BTreeMap<String, String>",
    into = "BTreeMap<String, String>"
)]
pub struct ResponseHeaders(BTreeMap<String, String>);

impl ResponseHeaders {
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<BTreeMap<String, String>> for ResponseHeaders {
    type Error = ValidationError;

    fn try_from(headers: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        if headers.len() > MAX_HEADERS {
            return Err(ValidationError::Proxy(format!(
                "at most {MAX_HEADERS} response headers"
            )));
        }
        let mut checked = BTreeMap::new();
        for (name, value) in headers {
            let name = name.to_ascii_lowercase();
            check_header(&name, &value)?;
            checked.insert(name, value);
        }
        Ok(Self(checked))
    }
}

impl From<ResponseHeaders> for BTreeMap<String, String> {
    fn from(headers: ResponseHeaders) -> Self {
        headers.0
    }
}

fn check_header(name: &str, value: &str) -> Result<(), ValidationError> {
    let token = |c: char| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c);
    if name.is_empty() || name.len() > MAX_HEADER_NAME_CHARS || !name.chars().all(token) {
        return Err(ValidationError::Proxy(format!(
            "invalid header name {name:?}"
        )));
    }
    if RESERVED_HEADERS.contains(&name) {
        return Err(ValidationError::Proxy(format!(
            "the proxy or the app sets {name}, it cannot be configured"
        )));
    }
    let printable = |c: char| c == ' ' || c == '\t' || c.is_ascii_graphic();
    if value.len() > MAX_HEADER_VALUE_CHARS || !value.chars().all(printable) {
        return Err(ValidationError::Proxy(format!(
            "invalid value for header {name}: use up to {MAX_HEADER_VALUE_CHARS} printable ASCII characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_change_nothing_and_read_from_empty_json() {
        let settings: ProxySettings = serde_json::from_str("{}").unwrap();
        assert!(settings.is_default());
    }

    #[test]
    fn reads_every_setting() {
        let settings: ProxySettings = serde_json::from_str(
            r#"{"rate_limit":{"per_second":10,"burst":20},"max_body_mb":5,"timeout_secs":300,
                "security_headers":true,"headers":{"Cache-Control":"no-store"},"maintenance":true,
                "cloudflare":true}"#,
        )
        .unwrap();
        let limit = settings.rate_limit.unwrap();
        assert_eq!((limit.per_second.get(), limit.burst.get()), (10, 20));
        assert_eq!(settings.max_body_mb.unwrap().get(), 5);
        assert_eq!(settings.timeout_secs.unwrap().get(), 300);
        assert_eq!(
            settings.headers.iter().collect::<Vec<_>>(),
            [("cache-control", "no-store")]
        );
        assert!(settings.security_headers && settings.maintenance && settings.cloudflare);
    }

    #[test]
    fn rejects_out_of_range_numbers_and_unsafe_headers() {
        for bad in [
            r#"{"rate_limit":{"per_second":0,"burst":1}}"#,
            r#"{"max_body_mb":20000}"#,
            r#"{"timeout_secs":0}"#,
            r#"{"headers":{"set-cookie":"a=b"}}"#,
            r#"{"headers":{"bad name":"x"}}"#,
            r#"{"headers":{"x-a":"line\nbreak"}}"#,
            r#"{"unknown":true}"#,
        ] {
            assert!(serde_json::from_str::<ProxySettings>(bad).is_err(), "{bad}");
        }
    }
}
