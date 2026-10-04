mod deploy;

use std::path::PathBuf;

use bird_core::{BackupId, DeploymentId, EnvKey, Hostname, ImageRef, Name, RegistryHost, Replicas};
use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub(crate) use deploy::DeployArgs;
use deploy::{parse_env, parse_port};

const EXAMPLES: &str = "\
Examples:
  bird init                    write a bird.toml for the app in this directory
  bird deploy                  deploy it, building the Dockerfile or pulling the image
  bird status                  see how it is doing
  bird logs -f                 follow its logs
  bird env set API_KEY='${{secret}}'
  bird add postgres            create a database, then link it:
  bird env set DATABASE_URL='${{postgres.DATABASE_URL}}'
  bird deploy web nginx:alpine --domain web.example.com

Commands act on the service named in ./bird.toml, or on the one given with -s <name>.";

#[derive(Debug, Parser)]
#[command(
    name = "bird",
    version,
    about = "Deploy and run apps on your bird server",
    after_help = EXAMPLES,
    disable_help_subcommand = true
)]
pub(crate) struct Args {
    /// Service to act on, defaults to the name in bird.toml
    #[arg(short, long, global = true, value_name = "NAME")]
    pub(crate) service: Option<Name>,
    /// Service manifest to read instead of ./bird.toml
    #[arg(short, long, global = true, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
    /// Print JSON for scripts instead of tables and messages
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// birdd API address, defaults to the logged in server or 127.0.0.1:7070
    #[arg(long, global = true, env = "BIRD_API", value_name = "HOST:PORT")]
    pub(crate) api: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Write a bird.toml for the app in this directory
    Init {
        /// Service name, defaults to the directory name
        name: Option<Name>,
        /// Image to run; left out, the Dockerfile here is built
        image: Option<ImageRef>,
        /// Port the app listens on, defaults to the Dockerfile's EXPOSE or 80
        #[arg(long, value_parser = parse_port)]
        port: Option<bird_core::Port>,
    },
    /// Deploy a service from bird.toml, an image or a Dockerfile, creating it if needed
    Deploy(Box<DeployArgs>),
    /// Show a service, its deployment and its machines
    Status,
    /// List all services
    #[command(visible_alias = "ls")]
    List,
    /// Show the logs of a service
    Logs {
        /// Lines to show from the end
        #[arg(short = 'n', long, default_value_t = 100)]
        tail: u32,
        /// Keep streaming new lines until interrupted
        #[arg(short, long)]
        follow: bool,
    },
    /// Manage a service's environment variables, lists them by default
    Env {
        #[command(subcommand)]
        command: Option<EnvCommand>,
    },
    /// Manage the domains routed to a service, lists them by default
    Domains {
        #[command(subcommand)]
        command: Option<DomainsCommand>,
    },
    /// Run a number of machines for a service
    Scale { replicas: Replicas },
    /// List the deployments of a service, newest first
    History,
    /// Redeploy an earlier deployment, the previous one by default
    Rollback { deployment: Option<DeploymentId> },
    /// Back up and restore the volumes of a service, lists backups by default
    Backup {
        #[command(subcommand)]
        command: Option<BackupCommand>,
    },
    /// Create a service from a template, like `bird add postgres`
    Add {
        template: Name,
        /// Name of the new service, defaults to the template name
        name: Option<Name>,
    },
    /// List templates for ready-made services like postgres and valkey
    Templates,
    /// Remove a service and destroy its machines
    #[command(visible_alias = "rm")]
    Remove {
        /// Also delete the service's volumes and all data on them; backups are kept
        #[arg(long)]
        purge: bool,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage credentials for private image registries, lists them by default
    Registry {
        #[command(subcommand)]
        command: Option<RegistryCommand>,
    },
    /// Save a birdd address and its API token, read from stdin
    Login {
        #[arg(value_name = "HOST:PORT")]
        api: String,
    },
    /// Print a shell completion script for bash, zsh, fish, elvish or powershell
    Completions { shell: Shell },
}

#[derive(Debug, Subcommand)]
pub(crate) enum EnvCommand {
    /// List variable names, values are never shown
    #[command(visible_alias = "ls")]
    List,
    /// Print the value of one variable
    Get {
        key: EnvKey,
        /// Print what the running deployment received, with references resolved
        #[arg(long)]
        deployed: bool,
    },
    /// Set variables as KEY=VALUE; values may use `${{service.KEY}}`, `${{KEY}}` and `${{secret}}`
    Set {
        #[arg(required = true, value_parser = parse_env, value_name = "KEY=VALUE")]
        variables: Vec<(EnvKey, String)>,
        /// Save without redeploying
        #[arg(long)]
        no_deploy: bool,
    },
    /// Remove variables
    Unset {
        #[arg(required = true)]
        keys: Vec<EnvKey>,
        /// Save without redeploying
        #[arg(long)]
        no_deploy: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BackupCommand {
    /// Copy every volume of a service; its machines pause while the copy runs
    Create,
    /// List backups of a service, also of one that was removed
    #[command(visible_alias = "ls")]
    List,
    /// Replace a service's volume data with a backup, saving the current data as a new backup first
    Restore {
        backup: BackupId,
        /// Restore data written by a different image than the service runs now
        #[arg(long)]
        allow_image_change: bool,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Delete a backup and its archives
    #[command(visible_alias = "rm")]
    Remove {
        backup: BackupId,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
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
    List,
    /// Route a domain to a service, https follows automatically when enabled
    Add { hostname: Hostname },
    /// Stop routing a domain to a service
    #[command(visible_alias = "rm")]
    Remove { hostname: Hostname },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn definitions_are_consistent() {
        Args::command().debug_assert();
    }

    #[test]
    fn service_comes_from_a_global_flag() {
        let args = Args::try_parse_from(["bird", "logs", "-f", "-s", "web", "-n", "20"]).unwrap();
        assert_eq!(args.service.unwrap().as_str(), "web");
        let Command::Logs { tail, follow } = args.command else {
            panic!("expected logs");
        };
        assert_eq!((tail, follow), (20, true));
        let args = Args::try_parse_from(["bird", "-s", "db", "status"]).unwrap();
        assert_eq!(args.service.unwrap().as_str(), "db");
        assert!(Args::try_parse_from(["bird", "status", "-s", "Web"]).is_err());
    }

    #[test]
    fn parses_env_commands() {
        let args = Args::try_parse_from(["bird", "env", "set", "A=1", "B=x=y"]).unwrap();
        let Command::Env {
            command:
                Some(EnvCommand::Set {
                    variables,
                    no_deploy,
                }),
        } = args.command
        else {
            panic!("expected env set");
        };
        assert_eq!(variables.len(), 2);
        assert_eq!(variables[1].1, "x=y");
        assert!(!no_deploy);
        assert!(Args::try_parse_from(["bird", "env", "set"]).is_err());
        assert!(Args::try_parse_from(["bird", "env", "unset", "bad-key"]).is_err());
        let args = Args::try_parse_from(["bird", "env"]).unwrap();
        assert!(matches!(args.command, Command::Env { command: None }));
    }

    #[test]
    fn parses_positional_values() {
        let args = Args::try_parse_from(["bird", "scale", "3"]).unwrap();
        assert!(matches!(args.command, Command::Scale { replicas } if replicas.get() == 3));
        assert!(Args::try_parse_from(["bird", "scale", "0"]).is_err());
        let args = Args::try_parse_from(["bird", "add", "postgres", "db"]).unwrap();
        assert!(
            matches!(args.command, Command::Add { name: Some(name), .. } if name.as_str() == "db")
        );
        let args = Args::try_parse_from(["bird", "rm", "--purge", "-y"]).unwrap();
        assert!(matches!(
            args.command,
            Command::Remove {
                purge: true,
                yes: true
            }
        ));
    }
}
