use bird_core::{EnvKey, Hostname, ImageRef, Name, Port};
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
    /// Deploy an image as a service, creating it if needed
    Deploy(DeployArgs),
    /// List services and their machines
    #[command(visible_alias = "ls")]
    List,
    /// Remove a service and destroy its machines
    #[command(visible_alias = "rm")]
    Remove { name: Name },
    /// Manage environment variables, changes redeploy the service
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    /// Show recent logs of a service
    Logs {
        name: Name,
        #[arg(long, default_value_t = 100)]
        tail: u32,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum EnvCommand {
    /// List variable names, values are never shown
    #[command(visible_alias = "ls")]
    List { name: Name },
    /// Set variables as KEY=VALUE
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

#[derive(Debug, clap::Args)]
pub(crate) struct DeployArgs {
    pub(crate) name: Name,
    pub(crate) image: ImageRef,
    /// Port the app listens on inside the container
    #[arg(long, default_value = "80", value_parser = parse_port)]
    pub(crate) port: Port,
    /// Domain to route to this service
    #[arg(long)]
    pub(crate) domain: Option<Hostname>,
    /// Environment variable as KEY=VALUE, repeatable
    #[arg(long = "env", short = 'e', value_parser = parse_env)]
    pub(crate) env: Vec<(EnvKey, String)>,
}

fn parse_port(raw: &str) -> Result<Port, String> {
    let number: u16 = raw.parse().map_err(|_| format!("{raw:?} is not a port"))?;
    Port::try_from(number).map_err(|err| err.to_string())
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
        assert_eq!(deploy.name.as_str(), "web");
        assert_eq!(deploy.port.get(), 80);
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
    fn rejects_invalid_values() {
        assert!(Args::try_parse_from(["bird", "deploy", "Web", "nginx"]).is_err());
        assert!(Args::try_parse_from(["bird", "deploy", "web", "nginx", "--port", "0"]).is_err());
        assert!(
            Args::try_parse_from(["bird", "deploy", "web", "nginx", "-e", "NOEQUALS"]).is_err()
        );
    }
}
