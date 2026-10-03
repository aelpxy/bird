use std::borrow::Cow;

use hyper::Request;
use hyper::header::{HOST, HeaderValue};

pub(crate) fn request_host<B>(request: &Request<B>) -> Option<Cow<'_, str>> {
    let raw = match request.uri().authority() {
        Some(authority) => authority.as_str(),
        None => request.headers().get(HOST)?.to_str().ok()?,
    };
    normalize(raw)
}

// routing uses an absolute target over Host (RFC 9112), so the app must see that same host
pub(crate) fn host_from_target<B>(request: &mut Request<B>) {
    let Some(authority) = request.uri().authority() else {
        return;
    };
    if let Ok(value) = HeaderValue::from_str(authority.as_str()) {
        request.headers_mut().insert(HOST, value);
    }
}

fn normalize(raw: &str) -> Option<Cow<'_, str>> {
    let host = strip_port(raw)?;
    let host = host.strip_suffix('.').unwrap_or(host);
    if host.is_empty() {
        return None;
    }
    if host.bytes().any(|b| b.is_ascii_uppercase()) {
        Some(Cow::Owned(host.to_ascii_lowercase()))
    } else {
        Some(Cow::Borrowed(host))
    }
}

fn strip_port(raw: &str) -> Option<&str> {
    if raw.starts_with('[') {
        let end = raw.find(']')?;
        return raw.get(..=end);
    }
    match raw.rsplit_once(':') {
        Some((host, port)) if port.bytes().all(|b| b.is_ascii_digit()) => Some(host),
        Some(_) => None,
        None => Some(raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_of(builder: hyper::http::request::Builder) -> Option<String> {
        request_host(&builder.body(()).unwrap()).map(Cow::into_owned)
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize("web.localhost").as_deref(), Some("web.localhost"));
        assert_eq!(
            normalize("Web.Localhost.").as_deref(),
            Some("web.localhost")
        );
        assert_eq!(
            normalize("web.localhost:8080").as_deref(),
            Some("web.localhost")
        );
        assert_eq!(normalize("[::1]:8080").as_deref(), Some("[::1]"));
        assert_eq!(normalize(""), None);
        assert_eq!(normalize(":80"), None);
        assert_eq!(normalize("a:b"), None);
    }

    #[test]
    fn borrows_when_already_normal() {
        assert!(matches!(normalize("web.localhost"), Some(Cow::Borrowed(_))));
    }

    #[test]
    fn prefers_absolute_uri_authority() {
        let builder = Request::builder()
            .uri("http://api.localhost/x")
            .header(HOST, "web.localhost");
        assert_eq!(host_of(builder).as_deref(), Some("api.localhost"));
    }

    #[test]
    fn absolute_target_replaces_host_header() {
        let mut request = Request::builder()
            .uri("http://api.localhost:8080/x")
            .header(HOST, "evil.example")
            .body(())
            .unwrap();
        host_from_target(&mut request);
        assert_eq!(request.headers()[HOST], "api.localhost:8080");
        let mut request = Request::builder()
            .uri("/x")
            .header(HOST, "web.localhost")
            .body(())
            .unwrap();
        host_from_target(&mut request);
        assert_eq!(request.headers()[HOST], "web.localhost");
    }

    #[test]
    fn falls_back_to_host_header() {
        let builder = Request::builder()
            .uri("/x")
            .header(HOST, "web.localhost:80");
        assert_eq!(host_of(builder).as_deref(), Some("web.localhost"));
        assert_eq!(host_of(Request::builder().uri("/x")), None);
    }
}
