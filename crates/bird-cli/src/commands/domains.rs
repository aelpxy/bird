use std::time::Duration;

use anyhow::Result;
use bird_api::AddDomain;
use bird_core::{Hostname, Name};

use crate::args::DomainsCommand;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(client: &ApiClient, command: DomainsCommand) -> Result<()> {
    match command {
        DomainsCommand::List { name } => {
            let hostnames: Vec<Hostname> = client
                .get(&format!("/v1/services/{name}/domains"), TIMEOUT)
                .await?;
            print_domains(&name, &hostnames);
        }
        DomainsCommand::Add { name, hostname } => {
            let hostnames: Vec<Hostname> = client
                .post(
                    &format!("/v1/services/{name}/domains"),
                    &AddDomain { hostname },
                    TIMEOUT,
                )
                .await?;
            print_domains(&name, &hostnames);
        }
        DomainsCommand::Remove { name, hostname } => {
            client
                .delete(&format!("/v1/services/{name}/domains/{hostname}"), TIMEOUT)
                .await?;
            println!("removed {hostname} from {name}");
        }
    }
    Ok(())
}

fn print_domains(name: &Name, hostnames: &[Hostname]) {
    if hostnames.is_empty() {
        println!("{name} has no domains");
    }
    for hostname in hostnames {
        println!("{hostname}");
    }
}
