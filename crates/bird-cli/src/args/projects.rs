use bird_core::Name;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// List projects and their environments
    #[command(visible_alias = "ls")]
    List,
    /// Create a project, with a `production` environment
    Create { name: Name },
    /// Delete a project; refused while any of its environments has services or backups
    #[command(visible_alias = "rm")]
    Remove {
        name: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum EnvironmentCommand {
    /// List the environments of the project
    #[command(visible_alias = "ls")]
    List,
    /// Create an environment in the project, with its own private network
    Create { name: Name },
    /// Delete an environment; refused while it has services or backups
    #[command(visible_alias = "rm")]
    Remove {
        name: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}
