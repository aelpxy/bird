use std::time::Duration;

use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::header::{HOST, HeaderValue};
use hyper::http::uri::{self, Authority, PathAndQuery};
use hyper::{Request, Response, StatusCode, Uri, Version};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioTimer};

use crate::body::{ProxyBody, error_response};
use crate::headers::{set_forwarded, strip_hop_by_hop};
use crate::host::request_host;
use crate::routes::{RouteError, Routes};
use crate::scheme::Scheme;
use crate::{ProxyConfig, Tls, edge};

pub(crate) struct Forwarder {
    routes: Routes,
    tls: Option<Tls>,
    client: Client<HttpConnector, Incoming>,
    response_timeout: Duration,
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
        Self {
            routes,
            tls,
            client,
            response_timeout: config.response_timeout,
        }
    }

    pub(crate) async fn handle(
        &self,
        mut request: Request<Incoming>,
        forwarded_for: Option<HeaderValue>,
        scheme: Scheme,
    ) -> Response<ProxyBody> {
        let http_edge = self.tls.as_ref().filter(|_| scheme == Scheme::Http);
        if let Some(response) =
            http_edge.and_then(|tls| edge::acme_response(&tls.challenges, request.uri().path()))
        {
            return response;
        }
        let Some(host) = request_host(&request) else {
            return error_response(StatusCode::BAD_REQUEST, "missing or invalid host\n");
        };
        if let Some(tls) = http_edge.filter(|tls| tls.certificates.contains(&host)) {
            return edge::https_redirect(&host, request.uri(), tls.https_port);
        }
        let upstream = match self.routes.pick(&host) {
            Ok(upstream) => upstream,
            Err(RouteError::UnknownHost) => {
                return error_response(StatusCode::NOT_FOUND, "no app is deployed at this host\n");
            }
            Err(RouteError::NoUpstreams) => {
                return error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "no machines are running for this app\n",
                );
            }
        };

        let Some(uri) = upstream_uri(request.uri(), upstream.clone()) else {
            return error_response(StatusCode::BAD_REQUEST, "invalid request target\n");
        };
        let original_host = request.headers().get(HOST).cloned();
        *request.uri_mut() = uri;
        *request.version_mut() = Version::HTTP_11;
        strip_hop_by_hop(request.headers_mut());
        set_forwarded(request.headers_mut(), forwarded_for, original_host, scheme);

        let method = request.method().clone();
        match tokio::time::timeout(self.response_timeout, self.client.request(request)).await {
            Ok(Ok(response)) => {
                let (mut parts, body) = response.into_parts();
                strip_hop_by_hop(&mut parts.headers);
                tracing::debug!(%method, %upstream, status = %parts.status, "proxied");
                Response::from_parts(parts, body.boxed())
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
