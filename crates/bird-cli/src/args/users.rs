use std::num::NonZeroU16;

use bird_core::Name;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum UserCommand {
    /// List users
    #[command(visible_alias = "ls")]
    List,
    /// Create a user and print their first token, shown only this once
    Create {
        name: Name,
        /// Let them manage users and everyone's tokens
        #[arg(long)]
        admin: bool,
    },
    /// Delete a user and their tokens
    #[command(visible_alias = "rm")]
    Remove {
        name: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TokenCommand {
    /// List tokens, without their secrets
    #[command(visible_alias = "ls")]
    List,
    /// Create a token and print it, shown only this once
    Create {
        name: Name,
        /// Days until it stops working; left out, it works until deleted
        #[arg(long, value_name = "DAYS")]
        expires_in_days: Option<NonZeroU16>,
    },
    /// Delete a token, which stops working at once
    #[command(visible_alias = "rm")]
    Remove {
        name: Name,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}
