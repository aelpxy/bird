use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{
    CreateToken, CreateUser, CreatedUser, IssuedToken, TokenSummary, UserSummary, Whoami,
};
use bird_core::{Name, UserRole};

use super::history::{ago, unix_now};
use super::table::render;
use crate::args::{TokenCommand, UserCommand};
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, prompt};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn whoami(client: &ApiClient, out: Output) -> Result<()> {
    let me = me(client).await?;
    if out.json(&me)? {
        return Ok(());
    }
    println!("{} ({})", me.name, me.role);
    Ok(())
}

pub(crate) async fn user(client: &ApiClient, command: UserCommand, out: Output) -> Result<()> {
    match command {
        UserCommand::List => {
            let users: Vec<UserSummary> = client.get("/v1/users", TIMEOUT).await?;
            if out.json(&users)? {
                return Ok(());
            }
            if users.is_empty() {
                println!("no users yet, add one with `bird user create <name>`");
                return Ok(());
            }
            let now = unix_now();
            let rows: Vec<Vec<String>> = users
                .into_iter()
                .map(|user| {
                    let yes_no = |on: bool| if on { "yes" } else { "no" }.to_owned();
                    vec![
                        user.name.to_string(),
                        user.role.to_string(),
                        yes_no(user.has_password),
                        yes_no(user.two_factor),
                        ago(now.saturating_sub(user.created_at)),
                    ]
                })
                .collect();
            print!(
                "{}",
                render(&["USER", "ROLE", "PASSWORD", "2FA", "CREATED"], &rows)
            );
        }
        UserCommand::Create { name, admin } => {
            let role = if admin {
                UserRole::Admin
            } else {
                UserRole::Member
            };
            let created: CreatedUser = client
                .post("/v1/users", &CreateUser { name, role }, TIMEOUT)
                .await?;
            if out.json(&created)? {
                return Ok(());
            }
            eprintln!(
                "{} created {} ({role}); their token follows and is not shown again; they log in with `bird login <host:port> --token`, then can set a password with `bird user passwd`",
                style::err(Paint::Green, "✓"),
                created.user.name
            );
            println!("{}", created.token.token);
        }
        UserCommand::Passwd { user } => return super::account::passwd(client, user).await,
        UserCommand::TwoFactor { command } => {
            return super::account::two_factor(client, command).await;
        }
        UserCommand::Remove { name, yes } => {
            prompt::confirm(
                &format!("delete user {name}? their tokens stop working at once"),
                yes,
            )?;
            client.delete(&format!("/v1/users/{name}"), TIMEOUT).await?;
            println!("{} deleted user {name}", style::out(Paint::Green, "✓"));
        }
    }
    Ok(())
}

pub(crate) async fn token(
    client: &ApiClient,
    user: Option<Name>,
    command: TokenCommand,
    out: Output,
) -> Result<()> {
    let user = match user {
        Some(user) => user,
        None => own_name(client).await?,
    };
    let path = format!("/v1/users/{user}/tokens");
    match command {
        TokenCommand::List => {
            let tokens: Vec<TokenSummary> = client.get(&path, TIMEOUT).await?;
            if out.json(&tokens)? {
                return Ok(());
            }
            if tokens.is_empty() {
                println!("{user} has no tokens, create one with `bird token create <name>`");
                return Ok(());
            }
            let now = unix_now();
            let rows: Vec<Vec<String>> = tokens
                .into_iter()
                .map(|token| {
                    vec![
                        token.name.to_string(),
                        format!("{}…", token.prefix),
                        ago(now.saturating_sub(token.created_at)),
                        token
                            .last_used_at
                            .map_or_else(|| "never".to_owned(), |at| ago(now.saturating_sub(at))),
                        token
                            .expires_at
                            .map_or_else(|| "never".to_owned(), |at| until(at.saturating_sub(now))),
                    ]
                })
                .collect();
            print!(
                "{}",
                render(&["TOKEN", "STARTS", "CREATED", "USED", "EXPIRES"], &rows)
            );
        }
        TokenCommand::Create {
            name,
            expires_in_days,
        } => {
            let request = CreateToken {
                name,
                expires_in_days,
            };
            let issued: IssuedToken = client.post(&path, &request, TIMEOUT).await?;
            if out.json(&issued)? {
                return Ok(());
            }
            eprintln!(
                "{} created token {} for {user}; it follows and is not shown again",
                style::err(Paint::Green, "✓"),
                issued.name
            );
            println!("{}", issued.token);
        }
        TokenCommand::Remove { name, yes } => {
            prompt::confirm(
                &format!("delete token {name} of {user}? it stops working at once"),
                yes,
            )?;
            client.delete(&format!("{path}/{name}"), TIMEOUT).await?;
            println!(
                "{} deleted token {name} of {user}",
                style::out(Paint::Green, "✓")
            );
        }
    }
    Ok(())
}

async fn me(client: &ApiClient) -> Result<Whoami> {
    client.get("/v1/me", TIMEOUT).await
}

// the token in birdd's data dir is root, which has no tokens of its own
async fn own_name(client: &ApiClient) -> Result<Name> {
    let me = me(client).await?;
    match me.name.parse() {
        Ok(name) if me.name != "root" => Ok(name),
        _ => bail!(
            "{} has no tokens of its own, name a user with --user",
            me.name
        ),
    }
}

fn until(seconds: i64) -> String {
    match seconds {
        ..=0 => "expired".to_owned(),
        1..3600 => format!("in {}m", seconds / 60),
        3600..86_400 => format!("in {}h", seconds / 3600),
        _ => format!("in {}d", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_time_left() {
        assert_eq!(until(-5), "expired");
        assert_eq!(until(120), "in 2m");
        assert_eq!(until(7200), "in 2h");
        assert_eq!(until(90 * 86_400), "in 90d");
    }
}
