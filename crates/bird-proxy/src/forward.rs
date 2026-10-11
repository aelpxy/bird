use std::error::Error;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use bird_core::Hostname;
use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::{Body, Incoming};
use hyper::header::{HOST, HeaderName, HeaderValue, UPGRADE};
use hyper::http::uri::{self, Authority, PathAndQuery};
use hyper::{Request, Response, StatusCode, Uri, Version};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioTimer};
use tokio::sync::OwnedSemaphorePermit;

use crate::body::{ProxyBody, error_response};
use crate::drain::Watch;
use crate::headers::{set_forwarded, strip_hop_by_hop};
use crate::host::{host_from_target, request_host};
use crate::idle::IdleTimeout;
use crate::policy::RoutePolicy;
use crate::routes::{RouteError, Routes, Target};
use crate::scheme::Scheme;
use crate::{ProxyConfig, Tls, edge, upgrade};

const X_MACHINE_ID: HeaderName = HeaderName::from_static("x-machine-id");

pub(crate) struct Forwarder {
    routes: Routes,
    tls: Option<Tls>,
    client: Client<HttpConnector, Limited<Incoming>>,
    limits: Limits,
    control: Option<Control>,
}

#[derive(Debug, Clone, Copy)]
struct Limits {
    response_timeout: Duration,
    body_idle_timeout: Duration,
    max_body_bytes: usize,
}

struct Control {
    host: Hostname,
    upstream: Authority,
    limits: Limits,
}

impl Forwarder {
    pub(crate) fn new(routes: Routes, tls: Option<Tls>, config: &ProxyConfig) -> Self {
        let mut connector = HttpConnector::new();
        connector.set_nodelay(true);
        connector.set_connect_timeout(Some(config.connect_timeout));
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .pool_timer(TokioTimer::new())
            .timer(TokioTimer::new())
            .build(connector);
        let control = config.control.as_ref().and_then(|route| {
            Some(Control {
                host: route.host.clone(),
                upstream: Authority::try_from(route.upstream.to_string()).ok()?,
                limits: Limits {
                    response_timeout: route.response_timeout,
                    body_idle_timeout: route.body_idle_timeout,
                    max_body_bytes: route.max_body_bytes,
                },
            })
        });
        Self {
            routes,
            tls,
            client,
            limits: Limits {
                response_timeout: config.response_timeout,
                body_idle_timeout: config.body_idle_timeout,
                max_body_bytes: config.max_body_bytes,
            },
            control,
        }
    }

    pub(crate) async fn handle(
        &self,
        mut request: Request<Incoming>,
        client_ip: IpAddr,
        forwarded_for: Option<HeaderValue>,
        scheme: Scheme,
        watch: &Watch,
        permit: &Arc<OwnedSemaphorePermit>,
    ) -> Response<ProxyBody> {
        host_from_target(&mut request);
        let http_edge = self.tls.as_ref().filter(|_| scheme == Scheme::Http);
        if let Some(response) =
            http_edge.and_then(|tls| edge::acme_response(&tls.challenges, request.uri().path()))
        {
            return response;
        }
        let Some(host) = request_host(&request) else {
            return error_response(StatusCode::BAD_REQUEST, "missing or invalid host\n");
        };
        let control = self
            .control
            .as_ref()
            .filter(|control| control.host.as_str() == host);
        // birdd's sign-in must never cross the network in the clear, so it is https before its certificate too
        if let Some(tls) =
            http_edge.filter(|tls| control.is_some() || tls.certificates.contains(&host))
        {
            return edge::https_redirect(&host, request.uri(), tls.https_port);
        }
        let dispatch = match control {
            Some(control) => Dispatch {
                upstream: control.upstream.clone(),
                machine: None,
                limits: control.limits,
                policy: None,
            },
            None => match self.dispatch(&host, request.uri(), scheme, client_ip) {
                Ok(dispatch) => dispatch,
                Err(response) => return *response,
            },
        };
        let Dispatch {
            upstream,
            machine,
            limits,
            policy,
        } = dispatch;

        if usize::try_from(request.body().size_hint().lower())
            .map_or(true, |length| length > limits.max_body_bytes)
        {
            return error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large\n");
        }
        let Some(uri) = upstream_uri(request.uri(), upstream.clone()) else {
            return error_response(StatusCode::BAD_REQUEST, "invalid request target\n");
        };
        let original_host = request.headers().get(HOST).cloned();
        let protocol = upgrade::requested_protocol(&request);
        let downstream = protocol.is_some().then(|| hyper::upgrade::on(&mut request));
        *request.uri_mut() = uri;
        *request.version_mut() = Version::HTTP_11;
        strip_hop_by_hop(request.headers_mut());
        if let Some(protocol) = protocol {
            upgrade::restore_headers(request.headers_mut(), protocol);
        }
        set_forwarded(request.headers_mut(), forwarded_for, original_host, scheme);

        let method = request.method().clone();
        let request = request.map(|body| Limited::new(body, limits.max_body_bytes));
        match tokio::time::timeout(limits.response_timeout, self.client.request(request)).await {
            Ok(Ok(mut response)) => {
                let tunnel =
                    downstream.filter(|_| response.status() == StatusCode::SWITCHING_PROTOCOLS);
                let accepted = response.headers().get(UPGRADE).cloned();
                if let Some(downstream) = tunnel {
                    let upstream = hyper::upgrade::on(&mut response);
                    upgrade::spawn_tunnel(downstream, upstream, watch.clone(), Arc::clone(permit));
                }
                let (mut parts, body) = response.into_parts();
                strip_hop_by_hop(&mut parts.headers);
                if let Some(accepted) =
                    accepted.filter(|_| parts.status == StatusCode::SWITCHING_PROTOCOLS)
                {
                    upgrade::restore_headers(&mut parts.headers, accepted);
                }
                if let Some(policy) = &policy {
                    policy.apply(&mut parts.headers, scheme);
                }
                if let Some(machine) = machine {
                    parts.headers.insert(X_MACHINE_ID, machine);
                }
                tracing::debug!(%method, %upstream, status = %parts.status, "proxied");
                Response::from_parts(
                    parts,
                    IdleTimeout::new(body, limits.body_idle_timeout).boxed(),
                )
            }
            Ok(Err(err)) if exceeded_body_limit(&err) => {
                error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large\n")
            }
            Ok(Err(err)) => {
                tracing::warn!(%method, %upstream, error = %err, "upstream request failed");
                error_response(StatusCode::BAD_GATEWAY, "upstream unavailable\n")
            }
            Err(_) => {
                tracing::warn!(%method, %upstream, "upstream timed out");
                error_response(StatusCode::GATEWAY_TIMEOUT, "upstream timed out\n")
            }
        }
    }
}

struct Dispatch {
    upstream: Authority,
    machine: Option<HeaderValue>,
    limits: Limits,
    policy: Option<Arc<RoutePolicy>>,
}

impl Forwarder {
    // the app's upstream with its own limits, or the answer the proxy gives in its place
    fn dispatch(
        &self,
        host: &str,
        uri: &Uri,
        scheme: Scheme,
        client_ip: IpAddr,
    ) -> Result<Dispatch, Box<Response<ProxyBody>>> {
        let (upstream, machine, policy) = match self.routes.pick(host) {
            Ok(Target::Upstream {
                authority,
                machine,
                policy,
            }) => (authority, machine, policy),
            Ok(Target::Redirect(target)) => {
                return Err(Box::new(edge::domain_redirect(
                    target.as_str(),
                    uri,
                    scheme,
                )));
            }
            Err(RouteError::UnknownHost) => {
                return Err(Box::new(error_response(
                    StatusCode::NOT_FOUND,
                    "no app is deployed at this host\n",
                )));
            }
            Err(RouteError::NoUpstreams) => {
                return Err(Box::new(error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "no machines are running for this app\n",
                )));
            }
        };
        let mut limits = self.limits;
        if let Some(policy) = &policy {
            if policy.maintenance {
                return Err(Box::new(edge::maintenance()));
            }
            if let Some(Err(wait)) = policy
                .rate_limit
                .as_ref()
                .map(|limit| limit.check(client_ip))
            {
                return Err(Box::new(edge::too_many_requests(wait)));
            }
            limits.max_body_bytes = policy.max_body_bytes.unwrap_or(limits.max_body_bytes);
            limits.response_timeout = policy.response_timeout.unwrap_or(limits.response_timeout);
        }
        Ok(Dispatch {
            upstream,
            machine,
            limits,
            policy,
        })
    }
}

fn exceeded_body_limit(err: &(dyn Error + 'static)) -> bool {
    std::iter::successors(Some(err), |&err| err.source()).any(<dyn Error>::is::<LengthLimitError>)
}

fn upstream_uri(original: &Uri, upstream: Authority) -> Option<Uri> {
    let path_and_query = original
        .path_and_query()
        .cloned()
        .unwrap_or_else(|| PathAndQuery::from_static("/"));
    Uri::builder()
        .scheme(uri::Scheme::HTTP)
        .authority(upstream)
        .path_and_query(path_and_query)
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_uri_to_upstream() {
        let upstream = Authority::from_static("127.0.0.1:4000");
        let uri = upstream_uri(&"/a/b?c=1".parse().unwrap(), upstream.clone()).unwrap();
        assert_eq!(uri, "http://127.0.0.1:4000/a/b?c=1");
        let uri = upstream_uri(&"http://web.localhost".parse().unwrap(), upstream).unwrap();
        assert_eq!(uri, "http://127.0.0.1:4000/");
    }
}
