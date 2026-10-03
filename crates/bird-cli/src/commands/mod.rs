mod deploy;
mod list;
mod logs;
mod remove;

use anyhow::Result;

use crate::args::{Args, Command};
use crate::client::ApiClient;

pub(crate) async fn run(args: Args) -> Result<()> {
    let client = ApiClient::new(args.api);
    match args.command {
        Command::Deploy(deploy) => deploy::run(&client, deploy).await,
        Command::List => list::run(&client).await,
        Command::Remove { name } => remove::run(&client, &name).await,
        Command::Logs { name, tail } => logs::run(&client, &name, tail).await,
    }
}
