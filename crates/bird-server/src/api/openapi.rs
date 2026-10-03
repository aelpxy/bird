use utoipa::openapi::OpenApi as Document;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

const TOKEN_SCHEME: &str = "api_token";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "bird",
        description = "Deploy and run containers on a bird server. Browse these docs at /docs. Every endpoint except the docs needs the API token from birdd's data directory as a bearer token."
    ),
    modifiers(&BearerToken),
    security(("api_token" = [])),
    tags(
        (name = "deployments", description = "Deploy images, inspect history and roll back"),
        (name = "services", description = "List, scale and remove services"),
        (name = "domains", description = "Route domains to services"),
        (name = "variables", description = "Environment variables, versioned with each deployment"),
        (name = "logs", description = "Recent and live logs of running machines"),
    )
)]
struct ApiDoc;

struct BearerToken;

impl Modify for BearerToken {
    fn modify(&self, document: &mut Document) {
        let components = document.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            TOKEN_SCHEME,
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
    }
}

pub(super) fn document() -> Document {
    ApiDoc::openapi()
}

pub(super) fn render(document: &Document) -> String {
    match document.to_pretty_json() {
        Ok(json) => json + "\n",
        Err(err) => {
            tracing::error!(error = %err, "could not render the openapi document");
            String::from("{}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::api::documented_routes;

    const UPDATE_ENV: &str = "BIRD_UPDATE_OPENAPI";

    fn committed_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/openapi.json")
    }

    // the committed document is part of the api contract, so drift fails the build
    #[test]
    fn committed_document_is_current() {
        let (_, spec) = documented_routes().split_for_parts();
        let rendered = render(&spec);
        let path = committed_path();
        if std::env::var_os(UPDATE_ENV).is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &rendered).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == rendered,
            "docs/openapi.json is out of date, run `{UPDATE_ENV}=1 cargo test -p bird-server`"
        );
    }

    #[test]
    fn documents_every_route_and_the_token() {
        let (_, spec) = documented_routes().split_for_parts();
        for path in [
            "/v1/deploy",
            "/v1/services",
            "/v1/services/{name}",
            "/v1/services/{name}/domains",
            "/v1/services/{name}/domains/{hostname}",
            "/v1/services/{name}/deployments",
            "/v1/services/{name}/rollback",
            "/v1/services/{name}/logs",
            "/v1/services/{name}/scale",
            "/v1/services/{name}/variables",
        ] {
            assert!(
                spec.paths.paths.contains_key(path),
                "{path} is not documented"
            );
        }
        let schemes = &spec.components.unwrap().security_schemes;
        assert!(schemes.contains_key(TOKEN_SCHEME));
    }
}
