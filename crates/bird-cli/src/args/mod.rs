mod deploy;
mod projects;

use std::path::PathBuf;

use bird_core::{
    BackupId, BackupInterval, BackupKeep, DeploymentId, EnvKey, Hostname, ImageRef, Name,
    RegistryHost, Replicas,
};
use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub(crate) use deploy::DeployArgs;
use deploy::{parse_env, parse_port};
pub(crate) use projects::{EnvironmentCommand, ProjectCommand};

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

Commands act on the service named in ./bird.toml, or on the one given with -s <name>, in the
project and environment from -p/-E, then bird.toml, then `bird switch`, else default/production.";

const EXEC_EXAMPLES: &str = "\
Examples:
  bird exec                            a shell in the service's first running machine
  bird exec -s db psql -U postgres     an interactive psql prompt
  bird exec -s db pg_dump -U postgres -Fc app > app.dump
  bird exec -s db pg_restore -U postgres -d app < app.dump
  bird exec -m a516d4 ls /data         in a specific machine

Ctrl-C and Ctrl-D go to the remote program; exit the shell or program to leave.";

const RUN_EXAMPLES: &str = "\
Examples:
  bird run                             a shell in a fresh container, removed when you leave
  bird run rails console               an interactive console with the service's variables
  bird run -T rake db:migrate          plain output, for scripts and deploy hooks

The container joins the private network but gets no volumes. The command goes to the image's
entrypoint as arguments, like `docker run`; pass --no-entrypoint to run it directly.";

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
    /// Project to act in, defaults to bird.toml, then `bird switch`, then `default`
    #[arg(short, long, global = true, env = "BIRD_PROJECT", value_name = "NAME")]
    pub(crate) project: Option<Name>,
    /// Environment to act in, defaults to bird.toml, then `bird switch`, then `production`
    #[arg(
        short = 'E',
        long,
        global = true,
        env = "BIRD_ENVIRONMENT",
        value_name = "NAME"
    )]
    pub(crate) environment: Option<Name>,
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
    /// Show a service, its deployment and its machines with their cpu, memory and network
    Status {
        /// Refresh every 2 seconds until interrupted
        #[arg(short, long)]
        watch: bool,
    },
    /// List the services in the environment
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
    /// Run a command or open a shell in a running machine; interactive when used from a terminal
    #[command(after_help = EXEC_EXAMPLES)]
    Exec {
        /// Machine to run in, as `bird status` shows it; defaults to the first running one
        #[arg(short, long)]
        machine: Option<String>,
        /// No terminal, even when run from one: piped stdin and the output pass through as bytes,
        /// with stderr kept apart
        #[arg(short = 'T', long)]
        no_tty: bool,
        /// Command and its arguments; left out, a shell
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "COMMAND"
        )]
        command: Vec<String>,
    },
    /// Run a command or open a shell in a new container from the service's image, with its
    /// variables and network but not its volumes; interactive when used from a terminal
    #[command(after_help = RUN_EXAMPLES)]
    Run {
        /// No terminal, even when run from one: piped stdin and the output pass through as bytes,
        /// with stderr kept apart
        #[arg(short = 'T', long)]
        no_tty: bool,
        /// Run the command itself; by default it goes to the image's entrypoint as arguments,
        /// like `docker run`
        #[arg(long)]
        no_entrypoint: bool,
        /// Command and its arguments; left out, a shell
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "COMMAND"
        )]
        command: Vec<String>,
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
    /// Stop a service; its machines, data and settings are kept until `bird start`
    Stop,
    /// Start a stopped service and wait until its machines are healthy
    Start,
    /// Restart a service's machines one at a time, each waiting to be healthy
    Restart,
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
    /// Manage projects, lists them by default
    Project {
        #[command(subcommand)]
        command: Option<ProjectCommand>,
    },
    /// Manage the environments of the project, lists them by default
    #[command(visible_alias = "environments")]
    Environment {
        #[command(subcommand)]
        command: Option<EnvironmentCommand>,
    },
    /// Act in another project and environment from now on, where bird.toml does not say
    Switch {
        project: Name,
        /// Defaults to `production`
        environment: Option<Name>,
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
    /// Back the volumes up on a schedule, like `--every 1d --keep 7`, or stop with `--off`
    Schedule {
        /// How often, from 1h to 30d, like 6h or 1d
        #[arg(long, required_unless_present = "off")]
        every: Option<BackupInterval>,
        /// How many scheduled backups to keep; manual and restore backups are never deleted
        #[arg(long, default_value = "7")]
        keep: BackupKeep,
        /// Stop the schedule, keeping the backups it made
        #[arg(long, conflicts_with = "every")]
        off: bool,
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
        let args = Args::try_parse_from(["bird", "status", "-w"]).unwrap();
        assert!(matches!(args.command, Command::Status { watch: true }));
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
    fn commands_keep_their_own_flags() {
        let args =
            Args::try_parse_from(["bird", "-s", "db", "exec", "psql", "-c", "select 1"]).unwrap();
        let Command::Exec {
            machine,
            no_tty,
            command,
        } = args.command
        else {
            panic!("expected exec");
        };
        assert_eq!((machine, no_tty), (None, false));
        assert_eq!(command, ["psql", "-c", "select 1"]);
        let args = Args::try_parse_from(["bird", "exec"]).unwrap();
        assert!(matches!(args.command, Command::Exec { command, .. } if command.is_empty()));
        let args = Args::try_parse_from(["bird", "exec", "-T", "env"]).unwrap();
        assert!(matches!(args.command, Command::Exec { no_tty: true, .. }));
        let args =
            Args::try_parse_from(["bird", "exec", "-m", "a516d4", "--", "ls", "-la"]).unwrap();
        assert!(matches!(args.command, Command::Exec { machine: Some(m), .. } if m == "a516d4"));
        let args = Args::try_parse_from(["bird", "run", "rake", "db:migrate", "--trace"]).unwrap();
        assert!(
            matches!(args.command, Command::Run { command, no_tty: false, .. } if command.len() == 3)
        );
        let args = Args::try_parse_from(["bird", "run"]).unwrap();
        assert!(matches!(args.command, Command::Run { command, .. } if command.is_empty()));
        let args = Args::try_parse_from(["bird", "run", "-T", "env"]).unwrap();
        assert!(matches!(args.command, Command::Run { no_tty: true, .. }));
    }

    #[test]
    fn parses_backup_schedules() {
        let args = Args::try_parse_from(["bird", "backup", "schedule", "--every", "6h"]).unwrap();
        let Command::Backup {
            command: Some(BackupCommand::Schedule { every, keep, off }),
        } = args.command
        else {
            panic!("expected backup schedule");
        };
        assert_eq!(every.map(BackupInterval::secs), Some(21_600));
        assert_eq!((keep.count(), off), (7, false));
        assert!(Args::try_parse_from(["bird", "backup", "schedule", "--off"]).is_ok());
        assert!(Args::try_parse_from(["bird", "backup", "schedule"]).is_err());
        assert!(
            Args::try_parse_from(["bird", "backup", "schedule", "--every", "1d", "--off"]).is_err()
        );
        assert!(Args::try_parse_from(["bird", "backup", "schedule", "--every", "10m"]).is_err());
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
