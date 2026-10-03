mod deploy;
mod domains;
mod env;
mod list;
mod login;
mod logs;
mod remove;
mod scale;

use anyhow::Result;

use crate::args::{Args, Command};
use crate::client::ApiClient;
use crate::profile;

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Login { api } => login::run(api).await,
        Command::Deploy(deploy) => deploy::run(&connect(args.api)?, deploy).await,
        Command::List => list::run(&connect(args.api)?).await,
        Command::Remove { name } => remove::run(&connect(args.api)?, &name).await,
        Command::Domains { command } => domains::run(&connect(args.api)?, command).await,
        Command::Scale { name, replicas } => scale::run(&connect(args.api)?, &name, replicas).await,
        Command::Env { command } => env::run(&connect(args.api)?, command).await,
        Command::Logs { name, tail } => logs::run(&connect(args.api)?, &name, tail).await,
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
