use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{
    DisableTwoFactor, RecoveryCodes, SetPassword, TwoFactorCode, TwoFactorSetup, Whoami,
};
use bird_core::Name;

use crate::args::TwoFactorCommand;
use crate::client::ApiClient;
use crate::ui::prompt;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn passwd(client: &ApiClient, user: Option<Name>) -> Result<()> {
    let me: Whoami = client.get("/v1/me", TIMEOUT).await?;
    let (user, yours) = target(&me, user)?;
    let current = if yours && me.has_password {
        Some(prompt::secret("current password: ")?)
    } else {
        None
    };
    let new = prompt::secret("new password: ")?;
    if prompt::secret("new password again: ")? != new {
        bail!("the two new passwords differ");
    }
    client
        .put(
            &format!("/v1/users/{user}/password"),
            &SetPassword { current, new },
            TIMEOUT,
        )
        .await?;
    let signed_out = if yours {
        "your other sessions are signed out"
    } else {
        "their sessions are signed out"
    };
    println!(
        "{} password set for {user}; {signed_out}",
        style::out(Paint::Green, "✓")
    );
    Ok(())
}

pub(crate) async fn two_factor(client: &ApiClient, command: TwoFactorCommand) -> Result<()> {
    let me: Whoami = client.get("/v1/me", TIMEOUT).await?;
    match command {
        TwoFactorCommand::Enable => {
            let (user, _) = target(&me, None)?;
            let setup: TwoFactorSetup = client
                .post(&format!("/v1/users/{user}/two-factor"), &(), TIMEOUT)
                .await?;
            eprintln!("add bird to your authenticator app with this link, or type the key in:");
            eprintln!("  {}", setup.uri);
            eprintln!("  key: {}", setup.secret);
            let code = prompt::line("then enter the code it shows: ")?;
            let recovery: RecoveryCodes = client
                .post(
                    &format!("/v1/users/{user}/two-factor/confirm"),
                    &TwoFactorCode { code },
                    TIMEOUT,
                )
                .await?;
            eprintln!(
                "{} two-factor is on; keep these recovery codes somewhere safe, each works once:",
                style::err(Paint::Green, "✓")
            );
            for code in recovery.codes {
                println!("{code}");
            }
        }
        TwoFactorCommand::Disable { user } => {
            let (user, yours) = target(&me, user)?;
            let password = if yours && me.has_password {
                Some(prompt::secret("password: ")?)
            } else {
                None
            };
            client
                .post_empty(
                    &format!("/v1/users/{user}/two-factor/disable"),
                    &DisableTwoFactor { password },
                    TIMEOUT,
                )
                .await?;
            println!(
                "{} two-factor is off for {user}",
                style::out(Paint::Green, "✓")
            );
        }
    }
    Ok(())
}

// the named user, else yourself; the token in birdd's data dir is root, which is no user
pub(crate) fn target(me: &Whoami, named: Option<Name>) -> Result<(Name, bool)> {
    if let Some(name) = named {
        let yours = name.as_str() == me.name;
        return Ok((name, yours));
    }
    match me.name.parse() {
        Ok(name) if me.name != "root" => Ok((name, true)),
        _ => bail!("{} is not a user, name one with --user", me.name),
    }
}
