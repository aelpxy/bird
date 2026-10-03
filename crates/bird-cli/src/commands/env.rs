use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{UpdateVariables, VariableValue, VariablesResponse};
use bird_core::{EnvKey, Name};

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use crate::args::EnvCommand;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(client: &ApiClient, command: EnvCommand) -> Result<()> {
    match command {
        EnvCommand::List { name } => list(client, &name).await,
        EnvCommand::Get {
            name,
            key,
            deployed,
        } => get(client, &name, &key, deployed).await,
        EnvCommand::Set {
            name,
            variables,
            no_deploy,
        } => {
            let update = UpdateVariables {
                set: variables.into_iter().collect(),
                unset: Vec::new(),
                deploy: !no_deploy,
            };
            apply(client, &name, &update).await
        }
        EnvCommand::Unset {
            name,
            keys,
            no_deploy,
        } => {
            let update = UpdateVariables {
                set: BTreeMap::new(),
                unset: keys,
                deploy: !no_deploy,
            };
            apply(client, &name, &update).await
        }
    }
}

async fn list(client: &ApiClient, name: &Name) -> Result<()> {
    let keys: Vec<EnvKey> = client
        .get(&format!("/v1/services/{name}/variables"), TIMEOUT)
        .await?;
    if keys.is_empty() {
        println!("{name} has no variables");
    }
    for key in keys {
        println!("{key}");
    }
    Ok(())
}

async fn get(client: &ApiClient, name: &Name, key: &EnvKey, deployed: bool) -> Result<()> {
    let variable: VariableValue = client
        .get(&format!("/v1/services/{name}/variables/{key}"), TIMEOUT)
        .await?;
    if !deployed {
        println!("{}", variable.value);
        return Ok(());
    }
    match variable.deployed {
        Some(value) => println!("{value}"),
        None => {
            bail!("{key} is not part of the running deployment of {name}, redeploy to apply it")
        }
    }
    Ok(())
}

async fn apply(client: &ApiClient, name: &Name, update: &UpdateVariables) -> Result<()> {
    if update.deploy {
        println!("updating variables and redeploying {name}...");
    }
    let response: VariablesResponse = client
        .patch(
            &format!("/v1/services/{name}/variables"),
            update,
            DEPLOY_TIMEOUT,
        )
        .await?;
    let keys: Vec<String> = response.keys.iter().map(ToString::to_string).collect();
    println!(
        "{name} variables: {}",
        if keys.is_empty() {
            "none".to_owned()
        } else {
            keys.join(", ")
        }
    );
    match &response.deployment {
        Some(deployment) => print_deployed(deployment),
        None => println!("saved, the changes apply on the next deploy"),
    }
    Ok(())
}
