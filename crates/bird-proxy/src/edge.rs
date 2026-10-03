use hyper::header::{HeaderValue, LOCATION};
use hyper::http::uri::PathAndQuery;
use hyper::{Response, StatusCode, Uri};

use crate::Challenges;
use crate::body::{ProxyBody, error_response, text_response};

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
