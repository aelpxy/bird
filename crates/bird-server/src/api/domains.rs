use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use std::collections::HashMap;

use bird_api::ErrorBody;
use bird_api::{AddDomain, DomainStatus, Https, UpdateDomain};
use bird_core::{Domain, Hostname, ServiceId};

use serde::Deserialize;

use super::scope::ServiceScope;
use crate::state::AppState;
use crate::tls::{CertFailures, Failure};
use crate::{Error, Result, routing};

const LOCAL_SUFFIX: &str = ".localhost";

/// List domains of a service
#[utoipa::path(get, operation_id = "list_domains", path = "/v1/projects/{project}/environments/{environment}/services/{name}/domains", tag = "domains", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name")), responses((status = 200, description = "Domains routed to the service, with where each one's certificate stands", body = Vec<DomainStatus>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
) -> Result<Json<Vec<DomainStatus>>> {
    let service = state.service(environment, &name).await?;
    let domains = state
        .db
        .call(move |store| store.list_domains(service.id))
        .await?;
    let certificates = state.db.call(|store| store.list_certificates(None)).await?;
    let expiry: HashMap<Hostname, i64> = certificates
        .into_iter()
        .map(|certificate| (certificate.hostname, certificate.not_after))
        .collect();
    let statuses = domains
        .into_iter()
        .map(|domain| DomainStatus {
            https: https(&domain.hostname, state.acme.as_deref(), &expiry),
            hostname: domain.hostname,
            redirect_to: domain.redirect_to,
        })
        .collect();
    Ok(Json(statuses))
}

fn https(
    hostname: &Hostname,
    acme: Option<&CertFailures>,
    expiry: &HashMap<Hostname, i64>,
) -> Https {
    if hostname.as_str().ends_with(LOCAL_SUFFIX) {
        return Https::Local;
    }
    let Some(failures) = acme else {
        return Https::Off;
    };
    if let Some(&expires_at) = expiry.get(hostname) {
        return Https::Issued { expires_at };
    }
    match failures.get(hostname) {
        Some(Failure { error, retry_at }) => Https::Failing { error, retry_at },
        None => Https::Pending,
    }
}

/// Route a domain to a service
#[utoipa::path(post, path = "/v1/projects/{project}/environments/{environment}/services/{name}/domains", tag = "domains", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name")), request_body = AddDomain, responses((status = 201, description = "Domain added, returns all domains of the service", body = Vec<String>), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn add(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Json(request): Json<AddDomain>,
) -> Result<(StatusCode, Json<Vec<Hostname>>)> {
    let service = state.service(environment, &name).await?;
    let service_id = service.id;
    let hostname = request.hostname;
    if state.dashboard_domain.as_ref() == Some(&hostname) {
        return Err(Error::DashboardDomain(hostname));
    }
    let claimed = hostname.clone();
    state
        .db
        .call(move |store| store.add_domain(service_id, &claimed))
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::DomainTaken(hostname),
            other => other,
        })?;
    applied(&state).await?;
    Ok((
        StatusCode::CREATED,
        Json(hostnames(&state, service_id).await?),
    ))
}

/// Send a domain's visitors to another domain of the service, or back to the app
#[utoipa::path(put, operation_id = "update_domain", path = "/v1/projects/{project}/environments/{environment}/services/{name}/domains/{hostname}", tag = "domains", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("hostname" = String, Path, description = "Domain to change")), request_body = UpdateDomain, responses((status = 204, description = "Saved and in effect"), (status = 400, description = "The target is not another domain of this service, or redirects itself", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn update(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(DomainPath { hostname }): Path<DomainPath>,
    Json(request): Json<UpdateDomain>,
) -> Result<StatusCode> {
    let service = state.service(environment, &name).await?;
    let domains = state
        .db
        .call(move |store| store.list_domains(service.id))
        .await?;
    check_redirect(&domains, &hostname, request.redirect_to.as_ref())?;
    state
        .db
        .call(move |store| store.set_domain_redirect(&hostname, request.redirect_to.as_ref()))
        .await?;
    routing::refresh(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}

// one hop only: a redirect lands on a domain that serves the app, so none can loop
fn check_redirect(domains: &[Domain], from: &Hostname, to: Option<&Hostname>) -> Result<()> {
    let Some(source) = domains.iter().find(|domain| &domain.hostname == from) else {
        return Err(Error::DomainNotFound(from.clone()));
    };
    let Some(to) = to else {
        return Ok(());
    };
    let invalid = |reason| Error::InvalidRedirect {
        from: source.hostname.clone(),
        to: to.clone(),
        reason,
    };
    if to == from {
        return Err(invalid("a domain cannot redirect to itself"));
    }
    let Some(target) = domains.iter().find(|domain| &domain.hostname == to) else {
        return Err(invalid("the target must be another domain of this service"));
    };
    if target.redirect_to.is_some() {
        return Err(invalid(
            "the target redirects too, point at a domain that serves the app",
        ));
    }
    if domains
        .iter()
        .any(|domain| domain.redirect_to.as_ref() == Some(from))
    {
        return Err(invalid(
            "other domains redirect here, point them elsewhere first",
        ));
    }
    Ok(())
}

/// Stop routing a domain to a service
#[utoipa::path(delete, operation_id = "remove_domain", path = "/v1/projects/{project}/environments/{environment}/services/{name}/domains/{hostname}", tag = "domains", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("hostname" = String, Path, description = "Domain to remove")), responses((status = 204, description = "Domain removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(DomainPath { hostname }): Path<DomainPath>,
) -> Result<StatusCode> {
    let service = state.service(environment, &name).await?;
    let service_id = service.id;
    let owned = hostnames(&state, service_id).await?.contains(&hostname);
    if !owned {
        return Err(Error::DomainNotFound(hostname));
    }
    state
        .db
        .call(move |store| store.remove_domain(&hostname))
        .await?;
    applied(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(crate) struct DomainPath {
    hostname: Hostname,
}

async fn applied(state: &AppState) -> Result<()> {
    routing::refresh(state).await?;
    state.domains_changed.notify_one();
    Ok(())
}

async fn hostnames(state: &AppState, service_id: ServiceId) -> Result<Vec<Hostname>> {
    let domains = state
        .db
        .call(move |store| store.list_domains(service_id))
        .await?;
    Ok(domains.into_iter().map(|d| d.hostname).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str) -> Hostname {
        name.parse().unwrap()
    }

    #[test]
    fn tells_where_each_certificate_stands() {
        let failures = CertFailures::default();
        let expiry = HashMap::from([(host("done.example.com"), 99)]);
        let failure = Failure {
            error: "dns problem".to_owned(),
            retry_at: 7,
        };
        failures.record(host("broken.example.com"), failure);
        let acme = Some(&failures);

        assert_eq!(
            https(&host("done.example.com"), acme, &expiry),
            Https::Issued { expires_at: 99 }
        );
        assert_eq!(
            https(&host("broken.example.com"), acme, &expiry),
            Https::Failing {
                error: "dns problem".to_owned(),
                retry_at: 7
            }
        );
        assert_eq!(
            https(&host("new.example.com"), acme, &expiry),
            Https::Pending
        );
        assert_eq!(https(&host("web.localhost"), acme, &expiry), Https::Local);
        assert_eq!(https(&host("done.example.com"), None, &expiry), Https::Off);
    }

    fn domain(name: &str, redirect_to: Option<&str>) -> Domain {
        Domain {
            hostname: host(name),
            service_id: ServiceId::generate(),
            redirect_to: redirect_to.map(host),
            created_at: 0,
        }
    }

    #[test]
    fn redirects_only_one_hop_within_the_service() {
        let domains = [
            domain("example.com", None),
            domain("www.example.com", None),
            domain("old.example.com", Some("example.com")),
        ];
        let www = host("www.example.com");
        let apex = host("example.com");
        assert!(check_redirect(&domains, &www, Some(&apex)).is_ok());
        assert!(check_redirect(&domains, &www, None).is_ok());
        for (from, to) in [
            ("www.example.com", "www.example.com"),
            ("www.example.com", "elsewhere.com"),
            ("www.example.com", "old.example.com"),
            ("example.com", "www.example.com"),
        ] {
            let result = check_redirect(&domains, &host(from), Some(&host(to)));
            assert!(
                matches!(result, Err(Error::InvalidRedirect { .. })),
                "{from} -> {to}"
            );
        }
        assert!(matches!(
            check_redirect(&domains, &host("nope.com"), None),
            Err(Error::DomainNotFound(_))
        ));
    }
}
