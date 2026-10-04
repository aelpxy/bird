mod backup;
mod build;
mod deploy;
mod domains;
mod env;
mod history;
mod init;
mod list;
mod login;
mod logs;
mod registry;
mod remove;
mod scale;
mod table;
mod templates;

use anyhow::Result;

use crate::args::{Args, Command};
use crate::client::ApiClient;
use crate::profile;

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Login { api } => login::run(api).await,
        Command::Deploy(deploy) => deploy::run(&connect(args.api)?, deploy).await,
        Command::Init { name, image, port } => init::run(&name, &image, port),
        Command::Registry { command } => registry::run(&connect(args.api)?, command).await,
        Command::Templates => templates::list(&connect(args.api)?).await,
        Command::Add { template, name } => {
            templates::add(&connect(args.api)?, &template, name).await
        }
        Command::List => list::run(&connect(args.api)?).await,
        Command::Remove { name, purge } => remove::run(&connect(args.api)?, &name, purge).await,
        Command::Domains { command } => domains::run(&connect(args.api)?, command).await,
        Command::History { name } => history::history(&connect(args.api)?, &name).await,
        Command::Rollback { name, deployment } => {
            history::rollback(&connect(args.api)?, &name, deployment).await
        }
        Command::Scale { name, replicas } => scale::run(&connect(args.api)?, &name, replicas).await,
        Command::Env { command } => env::run(&connect(args.api)?, command).await,
        Command::Backup { command } => backup::run(&connect(args.api)?, command).await,
        Command::Logs { name, tail, follow } => {
            logs::run(&connect(args.api)?, &name, tail, follow).await
        }
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
