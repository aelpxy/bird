use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{UpdateVariables, VariableValue, VariablesResponse};
use bird_core::{EnvKey, Name};

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use crate::args::EnvCommand;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, Spinner};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: EnvCommand,
    out: Output,
) -> Result<()> {
    match command {
        EnvCommand::List => list(client, name, out).await,
        EnvCommand::Get { key, deployed } => get(client, name, &key, deployed, out).await,
        EnvCommand::Set {
            variables,
            no_deploy,
        } => {
            let update = UpdateVariables {
                set: variables.into_iter().collect(),
                unset: Vec::new(),
                deploy: !no_deploy,
            };
            apply(client, name, &update, out).await
        }
        EnvCommand::Unset { keys, no_deploy } => {
            let update = UpdateVariables {
                set: BTreeMap::new(),
                unset: keys,
                deploy: !no_deploy,
            };
            apply(client, name, &update, out).await
        }
    }
}

async fn list(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let keys: Vec<EnvKey> = client
        .get(
            &client.scoped(&format!("services/{name}/variables")),
            TIMEOUT,
        )
        .await?;
    if out.json(&keys)? {
        return Ok(());
    }
    if keys.is_empty() {
        println!(
            "{name} has no variables, add some with {}",
            style::out(Paint::Bold, "bird env set KEY=value")
        );
    }
    for key in keys {
        println!("{key}");
    }
    Ok(())
}

async fn get(
    client: &ApiClient,
    name: &Name,
    key: &EnvKey,
    deployed: bool,
    out: Output,
) -> Result<()> {
    let variable: VariableValue = client
        .get(
            &client.scoped(&format!("services/{name}/variables/{key}")),
            TIMEOUT,
        )
        .await?;
    if out.json(&variable)? {
        return Ok(());
    }
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

async fn apply(
    client: &ApiClient,
    name: &Name,
    update: &UpdateVariables,
    out: Output,
) -> Result<()> {
    let spinner = Spinner::start(if update.deploy {
        format!("updating variables and redeploying {name}")
    } else {
        format!("updating variables of {name}")
    });
    let response: VariablesResponse = client
        .patch(
            &client.scoped(&format!("services/{name}/variables")),
            update,
            DEPLOY_TIMEOUT,
        )
        .await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    if out.json(&response)? {
        return Ok(());
    }
    let keys: Vec<String> = response.keys.iter().map(ToString::to_string).collect();
    println!(
        "{} {name} variables: {}",
        style::out(Paint::Green, "✓"),
        if keys.is_empty() {
            "none".to_owned()
        } else {
            keys.join(", ")
        }
    );
    if let Some(deployment) = &response.deployment {
        return print_deployed(deployment, elapsed, out);
    }
    println!(
        "  {}",
        style::out(Paint::Dim, "saved, the changes apply on the next deploy")
    );
    Ok(())
}
