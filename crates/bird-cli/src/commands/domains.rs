use std::time::Duration;

use anyhow::Result;
use bird_api::{AddDomain, DomainStatus, Https};
use bird_core::{Hostname, Name};

use super::table::render;
use crate::args::DomainsCommand;
use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: DomainsCommand,
    out: Output,
) -> Result<()> {
    match command {
        DomainsCommand::List => {
            let domains: Vec<DomainStatus> = client
                .get(&client.scoped(&format!("services/{name}/domains")), TIMEOUT)
                .await?;
            if !out.json(&domains)? {
                print_domains(name, &domains);
            }
        }
        DomainsCommand::Add { hostname } => {
            let hostnames: Vec<Hostname> = client
                .post(
                    &client.scoped(&format!("services/{name}/domains")),
                    &AddDomain {
                        hostname: hostname.clone(),
                    },
                    TIMEOUT,
                )
                .await?;
            if !out.json(&hostnames)? {
                println!(
                    "{} {hostname} routes to {name}; point its DNS at this server",
                    style::out(Paint::Green, "✓")
                );
            }
        }
        DomainsCommand::Remove { hostname } => {
            client
                .delete(
                    &client.scoped(&format!("services/{name}/domains/{hostname}")),
                    TIMEOUT,
                )
                .await?;
            println!(
                "{} {hostname} no longer routes to {name}",
                style::out(Paint::Green, "✓")
            );
        }
    }
    Ok(())
}

fn print_domains(name: &Name, domains: &[DomainStatus]) {
    if domains.is_empty() {
        println!(
            "{name} has no domains, add one with {}",
            style::out(Paint::Bold, "bird domains add <hostname>")
        );
        return;
    }
    let rows: Vec<Vec<String>> = domains
        .iter()
        .map(|domain| vec![domain.hostname.to_string(), https(&domain.https)])
        .collect();
    print!("{}", render(&["DOMAIN", "HTTPS"], &rows));
}

fn https(https: &Https) -> String {
    match https {
        Https::Issued { .. } => "issued".to_owned(),
        Https::Pending => "pending".to_owned(),
        Https::Failing { error, .. } => format!("failing: {error}"),
        Https::Off => "off".to_owned(),
        Https::Local => "local".to_owned(),
    }
}
