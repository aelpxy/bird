use std::num::NonZeroU16;

use bird_core::{Name, SessionId};
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
    /// Set a password: yours, asking for the current one, or anyone's for admins
    Passwd {
        /// Whose password; admins may reset anyone's, which signs them out everywhere
        #[arg(long, value_name = "NAME")]
        user: Option<Name>,
    },
    /// Turn two-factor sign-in on or off
    #[command(name = "2fa")]
    TwoFactor {
        #[command(subcommand)]
        command: TwoFactorCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TwoFactorCommand {
    /// Add bird to your authenticator app and get recovery codes
    Enable,
    /// Turn it off: yours with your password, or anyone's for admins
    Disable {
        #[arg(long, value_name = "NAME")]
        user: Option<Name>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SessionCommand {
    /// List where you are signed in
    #[command(visible_alias = "ls")]
    List,
    /// Sign a session out
    #[command(visible_alias = "rm")]
    Remove { id: SessionId },
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
