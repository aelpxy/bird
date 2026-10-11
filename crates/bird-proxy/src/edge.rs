use std::time::Duration;

use hyper::header::{HeaderValue, LOCATION, RETRY_AFTER};
use hyper::http::uri::PathAndQuery;
use hyper::{Response, StatusCode, Uri};

use crate::Challenges;
use crate::body::{ProxyBody, error_response, text_response};
use crate::scheme::Scheme;

const ACME_CHALLENGE_PREFIX: &str = "/.well-known/acme-challenge/";
const DEFAULT_HTTPS_PORT: u16 = 443;

pub(crate) fn acme_response(challenges: &Challenges, path: &str) -> Option<Response<ProxyBody>> {
    let token = path.strip_prefix(ACME_CHALLENGE_PREFIX)?;
    let key_authorization = challenges.key_authorization(token)?;
    Some(text_response(StatusCode::OK, key_authorization))
}

pub(crate) fn https_redirect(host: &str, uri: &Uri, https_port: u16) -> Response<ProxyBody> {
    let path = uri.path_and_query().map_or("/", PathAndQuery::as_str);
    let location = if https_port == DEFAULT_HTTPS_PORT {
        format!("https://{host}{path}")
    } else {
        format!("https://{host}:{https_port}{path}")
    };
    let Ok(location) = HeaderValue::try_from(location) else {
        return error_response(StatusCode::BAD_REQUEST, "invalid request target\n");
    };
    let mut response = text_response(StatusCode::PERMANENT_REDIRECT, "redirecting to https\n");
    response.headers_mut().insert(LOCATION, location);
    response
}

// a domain that only points visitors at another domain of the same app, like www to the apex
pub(crate) fn domain_redirect(target: &str, uri: &Uri, scheme: Scheme) -> Response<ProxyBody> {
    let path = uri.path_and_query().map_or("/", PathAndQuery::as_str);
    let Ok(location) = HeaderValue::try_from(format!("{}://{target}{path}", scheme.as_str()))
    else {
        return error_response(StatusCode::BAD_REQUEST, "invalid request target\n");
    };
    let mut response = text_response(StatusCode::PERMANENT_REDIRECT, "moved\n");
    response.headers_mut().insert(LOCATION, location);
    response
}

pub(crate) fn too_many_requests(wait: Duration) -> Response<ProxyBody> {
    // whole seconds, rounded up so a client that waits exactly this long gets through
    let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    let mut response = error_response(
        StatusCode::TOO_MANY_REQUESTS,
        "too many requests, slow down\n",
    );
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(seconds.max(1)));
    response
}

pub(crate) fn maintenance() -> Response<ProxyBody> {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "this app is down for maintenance, try again soon\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_known_acme_tokens_only() {
        let challenges = Challenges::new();
        challenges.insert("abc".to_owned(), "abc.thumb".to_owned());
        assert!(acme_response(&challenges, "/.well-known/acme-challenge/abc").is_some());
        assert!(acme_response(&challenges, "/.well-known/acme-challenge/nope").is_none());
        assert!(acme_response(&challenges, "/abc").is_none());
    }

    #[test]
    fn redirects_with_port_only_when_not_443() {
        let uri: Uri = "/a?b=1".parse().unwrap();
        let standard = https_redirect("web.example.com", &uri, 443);
        assert_eq!(standard.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            standard.headers()[LOCATION],
            "https://web.example.com/a?b=1"
        );
        let custom = https_redirect("web.example.com", &uri, 8443);
        assert_eq!(
            custom.headers()[LOCATION],
            "https://web.example.com:8443/a?b=1"
        );
    }
}
