use std::time::Duration;

use bird_core::Hostname;
use bird_proxy::{ControlRoute, ProxyConfig};

use crate::{Config, Error, Result};

// a deploy waits for its health check, which may take up to an hour, after pulling and releasing
const RESPONSE_TIMEOUT: Duration = Duration::from_hours(2);
// followed logs stay quiet while the app is, and the dashboard reconnects them anyway
const BODY_IDLE_TIMEOUT: Duration = Duration::from_hours(1);
// a folder deploy uploads its build context, which birdd itself caps at 512 MiB
const MAX_BODY_BYTES: usize = 512 * 1024 * 1024;
const LOCAL_SUFFIX: &str = ".localhost";

pub(crate) fn proxy_config(config: &Config) -> Result<ProxyConfig> {
    Ok(ProxyConfig {
        control: control_route(config)?,
        ..ProxyConfig::default()
    })
}

fn control_route(config: &Config) -> Result<Option<ControlRoute>> {
    let Some(host) = config.dashboard_domain.clone() else {
        return Ok(None);
    };
    if config.acme_directory.is_none() && !is_local(&host) {
        return Err(Error::DashboardNeedsTls(host));
    }
    Ok(Some(ControlRoute {
        host,
        upstream: config.api_addr,
        response_timeout: RESPONSE_TIMEOUT,
        body_idle_timeout: BODY_IDLE_TIMEOUT,
        max_body_bytes: MAX_BODY_BYTES,
    }))
}

fn is_local(host: &Hostname) -> bool {
    host.as_str().ends_with(LOCAL_SUFFIX)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn config(args: &[&str]) -> Config {
        Config::parse_from(std::iter::once("birdd").chain(args.iter().copied()))
    }

    #[test]
    fn routes_the_domain_to_the_api() {
        let route = control_route(&config(&[
            "--dashboard-domain",
            "bird.example.com",
            "--acme-directory",
            "https://ca/dir",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(route.host.as_str(), "bird.example.com");
        assert_eq!(route.upstream, "127.0.0.1:7070".parse().unwrap());
    }

    #[test]
    fn refuses_a_public_domain_without_https() {
        let err = control_route(&config(&["--dashboard-domain", "bird.example.com"])).unwrap_err();
        assert!(matches!(err, Error::DashboardNeedsTls(_)));
        let local = control_route(&config(&["--dashboard-domain", "bird.localhost"])).unwrap();
        assert!(local.is_some());
        assert!(control_route(&config(&[])).unwrap().is_none());
    }
}
