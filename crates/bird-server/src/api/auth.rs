use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use bird_api::ErrorBody;

use crate::token::ApiToken;

pub(crate) async fn require_token(
    State(token): State<ApiToken>,
    request: Request,
    next: Next,
) -> Response {
    match bearer(request.headers()) {
        Some(candidate) if token.matches(candidate) => next.run(request).await,
        _ => unauthorized(),
    }
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn unauthorized() -> Response {
    let body = ErrorBody {
        error: "missing or invalid api token".to_owned(),
        logs: Vec::new(),
    };
    (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, "Bearer")],
        Json(body),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer(&headers), None);
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&headers), None);
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer abc"));
        assert_eq!(bearer(&headers), Some("abc"));
    }
}
