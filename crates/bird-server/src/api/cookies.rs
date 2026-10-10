use std::time::Duration;

use axum::http::header::{COOKIE, HOST, ORIGIN, UPGRADE};
use axum::http::{HeaderMap, HeaderValue, Method};

pub(crate) const SESSION_COOKIE: &str = "bird_session";

// the session token a browser sent, if any; api tokens and the root token never travel this way
pub(crate) fn session(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| pair.trim().strip_prefix(SESSION_COOKIE)?.strip_prefix('='))
}

// strict same-site keeps other sites' requests from carrying it, http-only keeps scripts from
// reading it; plain http only works for a browser on this host
pub(crate) fn set_session(token: &str, lifetime: Duration, secure: bool) -> Option<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict{secure}",
        lifetime.as_secs()
    ))
    .ok()
}

#[must_use]
pub(crate) fn clear_session() -> HeaderValue {
    HeaderValue::from_static("bird_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict")
}

// a request that changes something or opens a terminal must come from a page birdd served:
// same-site is not enough when users' apps run on sibling subdomains of the dashboard
#[must_use]
pub(crate) fn cross_origin(method: &Method, headers: &HeaderMap) -> bool {
    let reads = matches!(*method, Method::GET | Method::HEAD) && !headers.contains_key(UPGRADE);
    !reads && !same_origin(headers)
}

fn same_origin(headers: &HeaderMap) -> bool {
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let (Some(origin), Some(host)) = (header(ORIGIN), header(HOST)) else {
        return false;
    };
    origin
        .split_once("://")
        .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn finds_the_session_among_other_cookies() {
        let sent = headers(&[
            ("cookie", "theme=dark; bird_sessionx=no"),
            ("cookie", "a=b; bird_session=birds_abc ; c=d"),
        ]);
        assert_eq!(session(&sent), Some("birds_abc"));
        assert_eq!(session(&headers(&[("cookie", "theme=dark")])), None);
        assert_eq!(session(&HeaderMap::new()), None);
    }

    #[test]
    fn marks_the_cookie_secure_off_this_host() {
        let local = set_session("birds_abc", Duration::from_secs(60), false).unwrap();
        assert_eq!(
            local,
            "bird_session=birds_abc; Path=/; Max-Age=60; HttpOnly; SameSite=Strict"
        );
        let remote = set_session("birds_abc", Duration::from_secs(60), true).unwrap();
        assert!(remote.to_str().unwrap().ends_with("; Secure"));
    }

    #[test]
    fn only_reads_may_come_from_elsewhere() {
        let page = [
            ("host", "bird.example.com"),
            ("origin", "https://bird.example.com"),
        ];
        let sibling = [
            ("host", "bird.example.com"),
            ("origin", "https://app.example.com"),
        ];
        assert!(!cross_origin(&Method::GET, &headers(&[])));
        assert!(!cross_origin(&Method::POST, &headers(&page)));
        assert!(cross_origin(&Method::POST, &headers(&sibling)));
        assert!(cross_origin(&Method::DELETE, &headers(&[("host", "a")])));
        let socket = [
            ("host", "bird.example.com"),
            ("origin", "https://app.example.com"),
            ("upgrade", "websocket"),
        ];
        assert!(cross_origin(&Method::GET, &headers(&socket)));
        let local = [
            ("host", "127.0.0.1:7070"),
            ("origin", "http://127.0.0.1:7070"),
        ];
        assert!(!cross_origin(&Method::PUT, &headers(&local)));
    }
}
