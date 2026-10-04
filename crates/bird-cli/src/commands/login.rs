use std::io::{IsTerminal, Write};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::{LoginRequest, LoginResponse, Whoami};
use bird_core::Name;

use crate::client::ApiClient;
use crate::profile::{self, Profile};
use crate::scope::Scope;
use crate::ui::prompt;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
// what session tokens start with; api tokens and the root token do not
const SESSION_PREFIX: &str = "birds_";

// from a terminal: username, password and a code if needed; piped or with --token: a token
pub(crate) async fn run(api: String, with_token: bool) -> Result<()> {
    let path = profile::path().context("cannot find a config directory, set HOME")?;
    let token = if with_token || !std::io::stdin().is_terminal() {
        read_token()?
    } else {
        sign_in(&api).await?
    };

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

pub(crate) async fn logout(client: &ApiClient) -> Result<()> {
    let path = profile::path().context("cannot find a config directory, set HOME")?;
    let Some(saved) = profile::load(&path)? else {
        bail!("not logged in");
    };
    if saved.token.starts_with(SESSION_PREFIX) {
        client.post_empty("/v1/logout", &(), TIMEOUT).await?;
    } else {
        eprintln!("the saved token keeps working until it is deleted with `bird token rm`");
    }
    std::fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    println!(
        "{} logged out of {}",
        style::out(Paint::Green, "✓"),
        saved.api
    );
    Ok(())
}

async fn sign_in(api: &str) -> Result<String> {
    let username: Name = prompt::line("username: ")?.trim().parse()?;
    let password = prompt::secret("password: ")?;
    let client = ApiClient::new(api.to_owned(), None, Scope::fallback());
    let mut request = LoginRequest {
        username,
        password,
        code: None,
    };
    for _ in 0..2 {
        match client.post("/v1/login", &request, TIMEOUT).await? {
            LoginResponse::SignedIn { token, .. } => return Ok(token),
            LoginResponse::TwoFactorRequired => {
                request.code = Some(prompt::line("two-factor code, or a recovery code: ")?);
            }
        }
    }
    bail!("birdd still asked for a two-factor code")
}

fn read_token() -> Result<String> {
    if std::io::stdin().is_terminal() {
        eprint!("api token: ");
        std::io::stderr().flush()?;
    }
    let token = prompt::line("")?.trim().to_owned();
    if token.is_empty() {
        bail!("no token given, pipe it in: bird login <host:port> < api-token");
    }
    Ok(token)
}
