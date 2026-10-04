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
        (name = "users", description = "Users and their API tokens"),
        (name = "orgs", description = "Orgs own projects; their members work in them"),
        (name = "projects", description = "Projects and their environments, each with its own private network"),
        (name = "deployments", description = "Deploy images, inspect history and roll back"),
        (name = "services", description = "List, scale and remove services"),
        (name = "domains", description = "Route domains to services"),
        (name = "variables", description = "Environment variables, versioned with each deployment"),
        (name = "logs", description = "Recent and live logs of running machines"),
        (name = "templates", description = "Ready-made services such as Postgres and Valkey"),
        (name = "registries", description = "Credentials for pulling private images"),
        (name = "builds", description = "Images built from source on the server"),
        (name = "backups", description = "Copies of service volumes, and restoring them"),
        (name = "commands", description = "One-off commands in a running machine or a fresh container"),
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
        let scoped =
            |rest: &str| format!("/v1/projects/{{project}}/environments/{{environment}}{rest}");
        let mut paths: Vec<String> = [
            "/deploy",
            "/services",
            "/services/{name}",
            "/services/{name}/domains",
            "/services/{name}/domains/{hostname}",
            "/services/{name}/deployments",
            "/services/{name}/rollback",
            "/services/{name}/logs",
            "/services/{name}/scale",
            "/services/{name}/variables",
            "/services/{name}/variables/{key}",
            "/services/{name}/builds",
            "/templates/{template}/deploy",
        ]
        .into_iter()
        .map(scoped)
        .collect();
        paths.extend(
            [
                "/v1/me",
                "/v1/orgs",
                "/v1/orgs/{org}",
                "/v1/orgs/{org}/members",
                "/v1/orgs/{org}/members/{user}",
                "/v1/users",
                "/v1/users/{user}",
                "/v1/users/{user}/tokens",
                "/v1/users/{user}/tokens/{token}",
                "/v1/projects",
                "/v1/projects/{project}",
                "/v1/projects/{project}/environments",
                "/v1/projects/{project}/environments/{environment}",
                "/v1/templates",
                "/v1/registries",
                "/v1/registries/{host}",
            ]
            .map(str::to_owned),
        );
        for path in &paths {
            assert!(
                spec.paths.paths.contains_key(path),
                "{path} is not documented"
            );
        }
        let schemes = &spec.components.unwrap().security_schemes;
        assert!(schemes.contains_key(TOKEN_SCHEME));
    }
}
