use std::path::PathBuf;

use bird_api::VolumeSpec;
use bird_core::{
    CpuLimit, EnvKey, HealthCheck, HealthTimeout, Hostname, ImageRef, MemoryLimit, Name, Port,
    Replicas,
};

const EXAMPLES: &str = "\
Examples:
  bird deploy                                  deploy the service in ./bird.toml
  bird deploy web nginx:alpine --domain web.example.com
  bird deploy api --build --port 3000          build ./Dockerfile on the server
  bird deploy db postgres:18 --port 5432 --health tcp -v data:/var/lib/postgresql";

#[derive(Debug, clap::Args)]
#[command(after_help = EXAMPLES)]
pub(crate) struct DeployArgs {
    /// Service name, defaults to -s or the name in bird.toml
    pub(crate) name: Option<Name>,
    /// Image to run, overrides bird.toml
    #[arg(conflicts_with = "build")]
    pub(crate) image: Option<ImageRef>,
    /// Build the image on the server from this directory's Dockerfile instead of pulling one
    #[arg(long, value_name = "DIR", num_args = 0..=1, default_missing_value = ".")]
    pub(crate) build: Option<PathBuf>,
    /// Dockerfile ARG as KEY=VALUE for --build or [build], repeatable; never put secrets here
    #[arg(long = "build-arg", value_parser = parse_env, value_name = "KEY=VALUE")]
    pub(crate) build_args: Vec<(EnvKey, String)>,
    /// Port the app listens on inside the container [default: 80]
    #[arg(long, value_parser = parse_port)]
    pub(crate) port: Option<Port>,
    /// Domain to route to this service, repeatable; leave out for internal-only services
    #[arg(long = "domain")]
    pub(crate) domains: Vec<Hostname>,
    /// How to tell the app is ready: http (any response, default for new services), tcp for
    /// databases, or a path like /healthz that must answer 2xx; left out, an existing service
    /// keeps its current check
    #[arg(long)]
    pub(crate) health: Option<HealthCheck>,
    /// How long a new machine has to pass its health check, like 90s or 5m (new services default to 60s)
    #[arg(long)]
    pub(crate) health_timeout: Option<HealthTimeout>,
    /// Environment variable as KEY=VALUE, repeatable
    #[arg(long = "env", short = 'e', value_parser = parse_env, value_name = "KEY=VALUE")]
    pub(crate) env: Vec<(EnvKey, String)>,
    /// Persistent volume as `NAME:/path`, repeatable; data survives redeploys
    #[arg(long = "volume", short = 'v', value_parser = parse_volume, value_name = "NAME:/PATH")]
    pub(crate) volumes: Vec<VolumeSpec>,
    /// Memory limit per machine, like 512m or 2g (new services default to 1g)
    #[arg(long)]
    pub(crate) memory: Option<MemoryLimit>,
    /// CPU limit per machine in cores, like 0.5 or 2 (new services default to 1)
    #[arg(long)]
    pub(crate) cpus: Option<CpuLimit>,
    /// Machines to run (new services default to 1)
    #[arg(long)]
    pub(crate) replicas: Option<Replicas>,
    /// Run an image whose version or base differs from the one that wrote the volume data
    #[arg(long)]
    pub(crate) allow_image_change: bool,
    /// Command to run instead of the image default, after `--`
    #[arg(last = true)]
    pub(crate) command: Vec<String>,
}

pub(super) fn parse_port(raw: &str) -> Result<Port, String> {
    let number: u16 = raw.parse().map_err(|_| format!("{raw:?} is not a port"))?;
    Port::try_from(number).map_err(|err| err.to_string())
}

fn parse_volume(raw: &str) -> Result<VolumeSpec, String> {
    let (name, path) = raw
        .split_once(':')
        .ok_or_else(|| format!("{raw:?} must look like NAME:/path"))?;
    Ok(VolumeSpec {
        name: name
            .parse()
            .map_err(|err: bird_core::ValidationError| err.to_string())?,
        path: path
            .parse()
            .map_err(|err: bird_core::ValidationError| err.to_string())?,
    })
}

pub(super) fn parse_env(raw: &str) -> Result<(EnvKey, String), String> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("{raw:?} must look like KEY=VALUE"))?;
    let key = key.parse::<EnvKey>().map_err(|err| err.to_string())?;
    Ok((key, value.to_owned()))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::args::{Args, Command};

    #[test]
    fn parses_deploy() {
        let args = Args::try_parse_from([
            "bird",
            "deploy",
            "web",
            "nginx:alpine",
            "--domain",
            "web.localhost",
            "-e",
            "A=1=2",
        ])
        .unwrap();
        let Command::Deploy(deploy) = args.command else {
            panic!("expected deploy");
        };
        assert_eq!(deploy.name.unwrap().as_str(), "web");
        assert_eq!(deploy.port, None);
        assert_eq!(deploy.domains.len(), 1);
        assert_eq!(deploy.env[0].1, "1=2");
    }

    #[test]
    fn parses_volumes() {
        let args = Args::try_parse_from([
            "bird",
            "deploy",
            "pg",
            "postgres:18",
            "-v",
            "data:/var/lib/postgresql",
        ])
        .unwrap();
        let Command::Deploy(deploy) = args.command else {
            panic!("expected deploy");
        };
        assert_eq!(deploy.volumes[0].name.as_str(), "data");
        assert_eq!(deploy.volumes[0].path.as_str(), "/var/lib/postgresql");
        for bad in ["data", "data:relative", "Data:/x", "data:/a/../b"] {
            assert!(
                Args::try_parse_from(["bird", "deploy", "pg", "postgres:18", "-v", bad]).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(Args::try_parse_from(["bird", "deploy", "Web", "nginx"]).is_err());
        assert!(Args::try_parse_from(["bird", "deploy", "web", "nginx", "--port", "0"]).is_err());
        assert!(
            Args::try_parse_from(["bird", "deploy", "web", "nginx", "-e", "NOEQUALS"]).is_err()
        );
    }
}
