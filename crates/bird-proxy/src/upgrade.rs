use hyper::header::{CONNECTION, HeaderValue, UPGRADE};
use hyper::upgrade::OnUpgrade;
use hyper::{HeaderMap, Request, Version};
use hyper_util::rt::TokioIo;

use crate::drain::Watch;

pub(crate) fn requested_protocol<B>(request: &Request<B>) -> Option<HeaderValue> {
    if request.version() != Version::HTTP_11 {
        return None;
    }
    let wants_upgrade = request
        .headers()
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
    if !wants_upgrade {
        return None;
    }
    request.headers().get(UPGRADE).cloned()
}

pub(crate) fn restore_headers(headers: &mut HeaderMap, protocol: HeaderValue) {
    headers.insert(CONNECTION, HeaderValue::from_static("upgrade"));
    headers.insert(UPGRADE, protocol);
}

pub(crate) fn spawn_tunnel(downstream: OnUpgrade, upstream: OnUpgrade, mut watch: Watch) {
    tokio::spawn(async move {
        let (downstream, upstream) = match tokio::try_join!(downstream, upstream) {
            Ok(pair) => pair,
            Err(err) => {
                tracing::debug!(error = %err, "upgrade did not complete");
                return;
            }
        };
        let mut downstream = TokioIo::new(downstream);
        let mut upstream = TokioIo::new(upstream);
        tokio::select! {
            copied = tokio::io::copy_bidirectional(&mut downstream, &mut upstream) => {
                if let Err(err) = copied {
                    tracing::debug!(error = %err, "tunnel closed with error");
                }
            }
            () = watch.signaled() => tracing::debug!("closing tunnel for shutdown"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(version: Version, connection: &str, upgrade: Option<&str>) -> Request<()> {
        let mut builder = Request::get("/")
            .version(version)
            .header(CONNECTION, connection);
        if let Some(upgrade) = upgrade {
            builder = builder.header(UPGRADE, upgrade);
        }
        builder.body(()).unwrap()
    }

    #[test]
    fn detects_upgrade_requests() {
        let ws = request(Version::HTTP_11, "keep-alive, Upgrade", Some("websocket"));
        assert_eq!(requested_protocol(&ws).unwrap(), "websocket");
    }

    #[test]
    fn ignores_incomplete_or_http10_upgrades() {
        assert!(
            requested_protocol(&request(Version::HTTP_11, "keep-alive", Some("websocket")))
                .is_none()
        );
        assert!(requested_protocol(&request(Version::HTTP_11, "upgrade", None)).is_none());
        assert!(
            requested_protocol(&request(Version::HTTP_10, "upgrade", Some("websocket"))).is_none()
        );
    }
}
