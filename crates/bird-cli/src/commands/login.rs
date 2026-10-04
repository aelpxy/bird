use std::io::{BufRead, IsTerminal, Write};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::Whoami;

use crate::client::ApiClient;
use crate::profile::{self, Profile};
use crate::scope::Scope;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(api: String) -> Result<()> {
    let path = profile::path().context("cannot find a config directory, set HOME")?;
    let token = read_token()?;

    let client = ApiClient::new(api.clone(), Some(token.clone()), Scope::fallback());
    let me: Whoami = client
        .get("/v1/me", TIMEOUT)
        .await
        .context("login failed")?;

    profile::save(
        &path,
        &Profile {
            api: api.clone(),
            token,
            project: None,
            environment: None,
        },
    )?;
    println!(
        "{} logged in to {api} as {}, saved to {}",
        style::out(Paint::Green, "✓"),
        me.name,
        path.display()
    );
    Ok(())
}

fn read_token() -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("api token: ");
        std::io::stderr().flush()?;
    }
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let token = line.trim().to_owned();
    if token.is_empty() {
        bail!("no token given, pipe it in: bird login <host:port> < api-token");
    }
    Ok(token)
}
