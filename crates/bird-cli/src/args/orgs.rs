use bird_core::{Name, OrgRole};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum OrgCommand {
    /// List your orgs and your role in each
    #[command(visible_alias = "ls")]
    List,
    /// Create an org, for server admins; you become its owner
    Create { name: Name },
    /// Delete an org that owns no projects
    Delete {
        name: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// List the members of an org
    Members { org: Name },
    /// Add a user to an org, or change their role there
    Add {
        org: Name,
        user: Name,
        /// member works with services, admin also manages projects and environments, owner also
        /// manages members
        #[arg(long, default_value = "member")]
        role: OrgRole,
    },
    /// Remove a user from an org, or leave one yourself
    Remove {
        org: Name,
        user: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}
