use std::io::{BufRead, IsTerminal, Write};
use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{RegistryLogin, RegistrySummary};

use super::table::render;
use crate::args::RegistryCommand;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(client: &ApiClient, command: RegistryCommand) -> Result<()> {
    match command {
        RegistryCommand::List => {
            let registries: Vec<RegistrySummary> = client.get("/v1/registries", TIMEOUT).await?;
            if registries.is_empty() {
                println!(
                    "no registries yet, add one with `bird registry login <host> --username <user>`"
                );
                return Ok(());
            }
            let rows: Vec<Vec<String>> = registries
                .into_iter()
                .map(|r| {
                    let mode = if r.insecure { "insecure" } else { "tls" };
                    vec![r.host.to_string(), r.username, mode.to_owned()]
                })
                .collect();
            print!("{}", render(&["REGISTRY", "USERNAME", "MODE"], &rows));
        }
        RegistryCommand::Login {
            host,
            username,
            insecure,
        } => {
            let password = read_password()?;
            let login = RegistryLogin {
                username,
                password,
                insecure,
            };
            client
                .put(&format!("/v1/registries/{host}"), &login, TIMEOUT)
                .await?;
            println!("stored credentials for {host}, deploys pull private images from it now");
        }
        RegistryCommand::Logout { host } => {
            client
                .delete(&format!("/v1/registries/{host}"), TIMEOUT)
                .await?;
            println!("removed credentials for {host}");
        }
    }
    Ok(())
}

// read from stdin so the password stays out of shell history and process lists
fn read_password() -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("password or token: ");
        std::io::stderr().flush()?;
    }
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let password = line.trim_end_matches(['\r', '\n']).to_owned();
    if password.is_empty() {
        bail!(
            "no password given, pipe it in: bird registry login <host> --username <user> < token"
        );
    }
    Ok(password)
}
