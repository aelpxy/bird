use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use bird_api::ErrorBody;
use bird_core::{Name, User, UserRole};
use tracing::Instrument;

use crate::db::Db;
use crate::token::ApiToken;
use crate::{Error, Result, users};

const ROOT: &str = "root";

#[derive(Clone)]
pub(crate) struct Auth {
    // the token in birdd's data dir, which always works, so nobody is locked out
    pub(crate) root: ApiToken,
    pub(crate) db: Db,
}

// who a request acts as, put in its extensions by `authenticate`
#[derive(Debug, Clone)]
pub(crate) enum Principal {
    Root,
    User(User),
}

impl Principal {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Root => ROOT,
            Self::User(user) => user.name.as_str(),
        }
    }

    pub(crate) const fn role(&self) -> UserRole {
        match self {
            Self::Root => UserRole::Admin,
            Self::User(user) => user.role,
        }
    }

    // `refusal` says what only server admins may do
    pub(crate) fn require_admin(&self, refusal: &'static str) -> Result<()> {
        match self.role() {
            UserRole::Admin => Ok(()),
            UserRole::Member => Err(Error::Forbidden(refusal)),
        }
    }

    // admins manage everyone's tokens, members only their own
    pub(crate) fn require_self_or_admin(&self, user: &Name) -> Result<()> {
        if self.role() == UserRole::Admin || self.name() == user.as_str() {
            Ok(())
        } else {
            Err(Error::Forbidden("you can only manage your own tokens"))
        }
    }
}

pub(crate) async fn authenticate(
    State(auth): State<Auth>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(candidate) = bearer(request.headers()) else {
        return unauthorized();
    };
    let principal = if auth.root.matches(candidate) {
        Principal::Root
    } else {
        let hashed = users::hash(candidate);
        match auth.db.call(move |store| store.authenticate(&hashed)).await {
            Ok(Some((user, token))) => {
                let db = auth.db.clone();
                tokio::spawn(async move {
                    if let Err(err) = db.call(move |store| store.record_token_use(token.id)).await {
                        tracing::debug!(error = %err, "could not record token use");
                    }
                });
                Principal::User(user)
            }
            Ok(None) => return unauthorized(),
            Err(err) => return err.into_response(),
        }
    };
    // every log line of the request says who made it
    let span = tracing::info_span!("request", user = principal.name());
    request.extensions_mut().insert(principal);
    next.run(request).instrument(span).await
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
        error: "missing, invalid or expired api token".to_owned(),
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
