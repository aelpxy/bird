use std::time::Duration;

use bird_core::ProxySettings;
use hyper::HeaderMap;
use hyper::header::{
    HeaderName, HeaderValue, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    X_FRAME_OPTIONS,
};

use crate::rate_limit::RateLimiter;
use crate::scheme::Scheme;

const BYTES_PER_MIB: usize = 1024 * 1024;

// only added when the app did not choose its own
const SECURITY_HEADERS: [(HeaderName, &str); 3] = [
    (X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (X_FRAME_OPTIONS, "SAMEORIGIN"),
    (REFERRER_POLICY, "strict-origin-when-cross-origin"),
];
const HSTS: &str = "max-age=31536000";

/// One app's proxy settings, ready for the request path; built once per settings change, so
/// its rate limiter keeps counting across route refreshes
#[derive(Debug)]
pub struct RoutePolicy {
    pub(crate) rate_limit: Option<RateLimiter>,
    pub(crate) max_body_bytes: Option<usize>,
    pub(crate) response_timeout: Option<Duration>,
    pub(crate) maintenance: bool,
    pub(crate) cloudflare: bool,
    security_headers: bool,
    headers: Vec<(HeaderName, HeaderValue)>,
}

impl RoutePolicy {
    #[must_use]
    pub fn new(settings: &ProxySettings) -> Self {
        Self {
            rate_limit: settings
                .rate_limit
                .map(|limit| RateLimiter::new(limit.per_second.get(), limit.burst.get())),
            max_body_bytes: settings.max_body_mb.and_then(|mb| {
                usize::try_from(mb.get())
                    .ok()
                    .map(|mb| mb.saturating_mul(BYTES_PER_MIB))
            }),
            response_timeout: settings
                .timeout_secs
                .map(|secs| Duration::from_secs(u64::from(secs.get()))),
            maintenance: settings.maintenance,
            cloudflare: settings.cloudflare,
            security_headers: settings.security_headers,
            // the settings were validated when saved, so nothing is dropped here in practice
            headers: settings
                .headers
                .iter()
                .filter_map(|(name, value)| {
                    Some((
                        HeaderName::try_from(name).ok()?,
                        HeaderValue::try_from(value).ok()?,
                    ))
                })
                .collect(),
        }
    }

    pub(crate) fn apply(&self, headers: &mut HeaderMap, scheme: Scheme) {
        if self.security_headers {
            for (name, value) in SECURITY_HEADERS {
                headers
                    .entry(name)
                    .or_insert(HeaderValue::from_static(value));
            }
            // browsers ignore it over http, and it must not pin a site that has no certificate
            if scheme == Scheme::Https {
                headers
                    .entry(STRICT_TRANSPORT_SECURITY)
                    .or_insert(HeaderValue::from_static(HSTS));
            }
        }
        for (name, value) in &self.headers {
            headers.insert(name.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(json: &str) -> RoutePolicy {
        RoutePolicy::new(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn turns_settings_into_limits() {
        let policy = policy(r#"{"max_body_mb":2,"timeout_secs":90,"maintenance":true}"#);
        assert_eq!(policy.max_body_bytes, Some(2 * BYTES_PER_MIB));
        assert_eq!(policy.response_timeout, Some(Duration::from_secs(90)));
        assert!(policy.maintenance && policy.rate_limit.is_none());
    }

    #[test]
    fn adds_security_headers_the_app_left_out_and_sets_custom_ones() {
        let policy = policy(r#"{"security_headers":true,"headers":{"cache-control":"no-store"}}"#);
        let mut headers = HeaderMap::new();
        headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
        headers.insert("cache-control", HeaderValue::from_static("max-age=60"));
        policy.apply(&mut headers, Scheme::Http);
        assert_eq!(headers[X_FRAME_OPTIONS], "DENY");
        assert_eq!(headers[X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers["cache-control"], "no-store");
        assert!(!headers.contains_key(STRICT_TRANSPORT_SECURITY));
        policy.apply(&mut headers, Scheme::Https);
        assert_eq!(headers[STRICT_TRANSPORT_SECURITY], HSTS);
    }
}
