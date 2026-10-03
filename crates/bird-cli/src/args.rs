use std::path::PathBuf;

use bird_api::VolumeSpec;
use bird_core::{
    CpuLimit, DeploymentId, EnvKey, HealthCheck, Hostname, ImageRef, MemoryLimit, Name, Port,
    RegistryHost, Replicas,
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "bird", version, about = "Deploy apps to your bird server")]
pub(crate) struct Args {
    /// birdd API address, defaults to the logged in server or 127.0.0.1:7070
    #[arg(long, global = true, env = "BIRD_API")]
    pub(crate) api: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Save a birdd address and API token, read from stdin
    Login { api: String },
    /// Deploy an image as a service, creating it if needed; reads ./bird.toml when present
    Deploy(DeployArgs),
    /// Write a starter bird.toml in the current directory
    Init {
        name: Name,
        image: ImageRef,
        #[arg(long, default_value = "80", value_parser = parse_port)]
        port: Port,
    },
    /// Manage credentials for private image registries
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// List templates for ready-made services like postgres and valkey
    Templates,
    /// Create a service from a template, such as `bird add postgres`
    Add {
        template: Name,
        /// Service name, defaults to the template name
        #[arg(long)]
        name: Option<Name>,
    },
    /// List services and their machines
    #[command(visible_alias = "ls")]
    List,
    /// Remove a service and destroy its machines
    #[command(visible_alias = "rm")]
    Remove {
        name: Name,
        /// Also delete the service's volumes and all data on them
        #[arg(long)]
        purge: bool,
    },
    /// Manage environment variables, changes redeploy the service
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    /// Manage the domains routed to a service
    Domains {
        #[command(subcommand)]
        command: DomainsCommand,
    },
    /// List the deployments of a service, newest first
    History { name: Name },
    /// Redeploy an earlier deployment, the previous one by default
    Rollback {
        name: Name,
        deployment: Option<DeploymentId>,
    },
    /// Run a number of machines for a service
    Scale { name: Name, replicas: Replicas },
    /// Show recent logs of a service
    Logs {
        name: Name,
        #[arg(long, default_value_t = 100)]
        tail: u32,
        /// Keep streaming new lines until interrupted
        #[arg(short, long)]
        follow: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum EnvCommand {
    /// List variable names, values are never shown
    #[command(visible_alias = "ls")]
    List { name: Name },
    /// Print the value of one variable
    Get {
        name: Name,
        key: EnvKey,
        /// Print what the running deployment received, with references resolved
        #[arg(long)]
        deployed: bool,
    },
    /// Set variables as KEY=VALUE; values may use `${{service.KEY}}`, `${{KEY}}` and `${{secret}}`
    Set {
        name: Name,
        #[arg(required = true, value_parser = parse_env)]
        variables: Vec<(EnvKey, String)>,
        /// Save without redeploying
        #[arg(long)]
        no_deploy: bool,
    },
    /// Remove variables
    Unset {
        name: Name,
        #[arg(required = true)]
        keys: Vec<EnvKey>,
        /// Save without redeploying
        #[arg(long)]
        no_deploy: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum RegistryCommand {
    /// List registries with stored credentials
    #[command(visible_alias = "ls")]
    List,
    /// Store credentials for a registry; the password or token is read from stdin
    Login {
        host: RegistryHost,
        #[arg(long)]
        username: String,
        /// Plain http without certificate checks, only for registries on a private network
        #[arg(long)]
        insecure: bool,
    },
    /// Forget the credentials of a registry
    Logout { host: RegistryHost },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DomainsCommand {
    /// List domains of a service
    #[command(visible_alias = "ls")]
    List { name: Name },
    /// Route a domain to a service, https follows automatically when enabled
    Add { name: Name, hostname: Hostname },
    /// Stop routing a domain to a service
    #[command(visible_alias = "rm")]
    Remove { name: Name, hostname: Hostname },
}

#[derive(Debug, clap::Args)]
pub(crate) struct DeployArgs {
    /// Service name, overrides bird.toml
    pub(crate) name: Option<Name>,
    /// Image to run, overrides bird.toml
    #[arg(conflicts_with = "build")]
    pub(crate) image: Option<ImageRef>,
    /// Build the image on the server from this directory's Dockerfile instead of pulling one
    #[arg(long, value_name = "DIR", num_args = 0..=1, default_missing_value = ".")]
    pub(crate) build: Option<PathBuf>,
    /// Dockerfile ARG as KEY=VALUE for --build or [build], repeatable; never put secrets here
    #[arg(long = "build-arg", value_parser = parse_env)]
    pub(crate) build_args: Vec<(EnvKey, String)>,
    /// Service manifest to read instead of ./bird.toml
    #[arg(long, short = 'c')]
    pub(crate) config: Option<PathBuf>,
    /// Port the app listens on inside the container [default: 80]
    #[arg(long, value_parser = parse_port)]
    pub(crate) port: Option<Port>,
    /// Domain to route to this service, repeatable; leave out for internal-only services
    #[arg(long = "domain")]
    pub(crate) domains: Vec<Hostname>,
    /// How to tell the app is ready: http (default for new services) or tcp for databases;
    /// left out, an existing service keeps its current check
    #[arg(long)]
    pub(crate) health: Option<HealthCheck>,
    /// Environment variable as KEY=VALUE, repeatable
    #[arg(long = "env", short = 'e', value_parser = parse_env)]
    pub(crate) env: Vec<(EnvKey, String)>,
    /// Persistent volume as `NAME:/path`, repeatable; data survives redeploys
    #[arg(long = "volume", short = 'v', value_parser = parse_volume)]
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

fn parse_port(raw: &str) -> Result<Port, String> {
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

fn parse_env(raw: &str) -> Result<(EnvKey, String), String> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("{raw:?} must look like KEY=VALUE"))?;
    let key = key.parse::<EnvKey>().map_err(|err| err.to_string())?;
    Ok((key, value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_env_commands() {
        let args = Args::try_parse_from(["bird", "env", "set", "web", "A=1", "B=x=y"]).unwrap();
        let Command::Env {
            command:
                EnvCommand::Set {
                    variables,
                    no_deploy,
                    ..
                },
        } = args.command
        else {
            panic!("expected env set");
        };
        assert_eq!(variables.len(), 2);
        assert!(!no_deploy);
        assert!(Args::try_parse_from(["bird", "env", "set", "web"]).is_err());
        assert!(Args::try_parse_from(["bird", "env", "unset", "web", "bad-key"]).is_err());
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
