mod backup;
mod build;
mod completions;
mod deploy;
mod domains;
mod env;
mod exec;
mod history;
mod init;
mod list;
mod login;
mod logs;
mod power;
mod registry;
mod remove;
mod scale;
mod status;
mod table;
mod templates;
mod tty;
mod watch;

use std::path::Path;

use anyhow::Result;
use bird_core::Name;

use crate::args::{Args, BackupCommand, Command, DomainsCommand, EnvCommand, RegistryCommand};
use crate::client::ApiClient;
use crate::ui::Output;
use crate::{manifest, profile};

pub(crate) use exec::RemoteExit;

pub(crate) async fn run(args: Args) -> Result<()> {
    let Args {
        service,
        config,
        json,
        api,
        command,
    } = args;
    let out = Output { json };
    let target = || target(service.clone(), config.as_deref());
    match command {
        Command::Init { name, image, port } => init::run(name, image.as_ref(), port),
        Command::Deploy(deploy) => {
            deploy::run(&connect(api)?, *deploy, service, config.as_deref(), out).await
        }
        Command::Status { watch } => status::run(&connect(api)?, &target()?, watch, out).await,
        Command::List => list::run(&connect(api)?, out).await,
        Command::Logs { tail, follow } => {
            logs::run(&connect(api)?, &target()?, tail, follow, out).await
        }
        Command::Exec {
            machine,
            no_tty,
            command,
        } => {
            let client = connect(api)?;
            let name = target()?;
            if !no_tty && !out.json && crate::ui::terminal::interactive() {
                let place = tty::Place::Machine(machine.as_deref());
                tty::session(&client, &name, place, command).await
            } else {
                exec::exec(&client, &name, machine, command, out).await
            }
        }
        Command::Run { no_tty, command } => {
            let client = connect(api)?;
            let name = target()?;
            if !no_tty && !out.json && crate::ui::terminal::interactive() {
                tty::session(&client, &name, tty::Place::NewContainer, command).await
            } else {
                exec::run(&client, &name, command, out).await
            }
        }
        Command::Env { command } => {
            let command = command.unwrap_or(EnvCommand::List);
            env::run(&connect(api)?, &target()?, command, out).await
        }
        Command::Domains { command } => {
            let command = command.unwrap_or(DomainsCommand::List);
            domains::run(&connect(api)?, &target()?, command, out).await
        }
        Command::Scale { replicas } => scale::run(&connect(api)?, &target()?, replicas).await,
        Command::Stop => power::stop(&connect(api)?, &target()?, out).await,
        Command::Start => power::start(&connect(api)?, &target()?, out).await,
        Command::Restart => power::restart(&connect(api)?, &target()?, out).await,
        Command::History => history::history(&connect(api)?, &target()?, out).await,
        Command::Rollback { deployment } => {
            history::rollback(&connect(api)?, &target()?, deployment, out).await
        }
        Command::Backup { command } => {
            let command = command.unwrap_or(BackupCommand::List);
            backup::run(&connect(api)?, &target()?, command, out).await
        }
        Command::Add { template, name } => {
            templates::add(&connect(api)?, &template, name, out).await
        }
        Command::Templates => templates::list(&connect(api)?, out).await,
        Command::Remove { purge, yes } => remove::run(&connect(api)?, &target()?, purge, yes).await,
        Command::Registry { command } => {
            let command = command.unwrap_or(RegistryCommand::List);
            registry::run(&connect(api)?, command, out).await
        }
        Command::Login { api } => login::run(api).await,
        Command::Completions { shell } => completions::run(shell),
    }
}

// -s wins, then the name in bird.toml, so commands run inside an app's directory need no name
fn target(explicit: Option<Name>, config: Option<&Path>) -> Result<Name> {
    if let Some(name) = explicit {
        return Ok(name);
    }
    match manifest::load(config)? {
        Some(loaded) => Ok(loaded.manifest.name),
        None => anyhow::bail!(
            "which service? pass -s <name>, or run this where a {} names one",
            bird_api::MANIFEST_FILE
        ),
    }
}

fn connect(api_override: Option<String>) -> Result<ApiClient> {
    let saved = match profile::path() {
        Some(path) => profile::load(&path)?,
        None => None,
    };
    let env_token = std::env::var(profile::TOKEN_ENV).ok();
    let target = profile::resolve(api_override, env_token, saved);
    Ok(ApiClient::new(target.api, target.token))
}
