use std::net::IpAddr;

use hyper::HeaderMap;
use hyper::header::{
    CONNECTION, FORWARDED, HeaderName, HeaderValue, TE, TRAILER, TRANSFER_ENCODING, UPGRADE,
};

use crate::scheme::Scheme;

const HOP_BY_HOP: [HeaderName; 7] = [
    CONNECTION,
    HeaderName::from_static("keep-alive"),
    HeaderName::from_static("proxy-connection"),
    TE,
    TRAILER,
    TRANSFER_ENCODING,
    UPGRADE,
];

const X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
const X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
const X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
const X_SERVER: HeaderName = HeaderName::from_static("x-server");

pub(crate) fn strip_hop_by_hop(headers: &mut HeaderMap) {
    // one cheap pass instead of seven hashed removals, since these headers are rarely present
    if !headers.keys().any(|name| HOP_BY_HOP.contains(name)) {
        return;
    }
    let listed: Vec<HeaderName> = headers
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
        .collect();
    for name in listed {
        headers.remove(name);
    }
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
}

pub(crate) fn forwarded_for(client_ip: IpAddr) -> Option<HeaderValue> {
    HeaderValue::try_from(client_ip.to_string()).ok()
}

// we are the edge, so client-supplied forwarding headers are untrusted and replaced
pub(crate) fn set_forwarded(
    headers: &mut HeaderMap,
    forwarded_for: Option<HeaderValue>,
    original_host: Option<HeaderValue>,
    scheme: Scheme,
) {
    headers.remove(FORWARDED);
    match forwarded_for {
        Some(value) => headers.insert(X_FORWARDED_FOR, value),
        None => headers.remove(X_FORWARDED_FOR),
    };
    headers.insert(X_FORWARDED_PROTO, HeaderValue::from_static(scheme.as_str()));
    match original_host {
        Some(host) => headers.insert(X_FORWARDED_HOST, host),
        None => headers.remove(X_FORWARDED_HOST),
    };
}

// replaces whatever the app sent, so every response is marked as served by bird
pub(crate) fn set_server(headers: &mut HeaderMap) {
    headers.insert(X_SERVER, HeaderValue::from_static("Bird"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_standard_and_listed_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(CONNECTION, HeaderValue::from_static("close, X-Secret"));
        headers.insert("x-secret", HeaderValue::from_static("1"));
        headers.insert("keep-alive", HeaderValue::from_static("timeout=5"));
        headers.insert(UPGRADE, HeaderValue::from_static("websocket"));
        headers.insert("x-keep", HeaderValue::from_static("1"));
        strip_hop_by_hop(&mut headers);
        assert_eq!(headers.len(), 1);
        assert!(headers.contains_key("x-keep"));
    }

    #[test]
    fn leaves_clean_headers_untouched() {
        let mut headers = HeaderMap::new();
        headers.insert("x-keep", HeaderValue::from_static("1"));
        headers.insert(hyper::header::ACCEPT, HeaderValue::from_static("*/*"));
        strip_hop_by_hop(&mut headers);
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn replaces_spoofed_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, HeaderValue::from_static("6.6.6.6"));
        headers.insert(FORWARDED, HeaderValue::from_static("for=6.6.6.6"));
        headers.insert(X_FORWARDED_HOST, HeaderValue::from_static("evil"));
        set_forwarded(
            &mut headers,
            forwarded_for("10.0.0.1".parse().unwrap()),
            Some(HeaderValue::from_static("web.localhost")),
            Scheme::Https,
        );
        assert_eq!(headers[X_FORWARDED_FOR], "10.0.0.1");
        assert_eq!(headers[X_FORWARDED_HOST], "web.localhost");
        assert_eq!(headers[X_FORWARDED_PROTO], "https");
        assert!(!headers.contains_key(FORWARDED));
    }

    #[test]
    fn overrides_app_server_header() {
        let mut headers = HeaderMap::new();
        headers.append(X_SERVER, HeaderValue::from_static("custom"));
        headers.append(X_SERVER, HeaderValue::from_static("other"));
        set_server(&mut headers);
        assert_eq!(
            headers.get_all(X_SERVER).iter().collect::<Vec<_>>(),
            ["Bird"]
        );
    }
}
