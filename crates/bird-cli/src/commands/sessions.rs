use std::time::Duration;

use anyhow::Result;
use bird_api::{SessionSummary, Whoami};
use bird_core::Name;

use super::account::target;
use super::history::{ago, unix_now};
use super::table::render;
use crate::args::SessionCommand;
use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(
    client: &ApiClient,
    user: Option<Name>,
    command: SessionCommand,
    out: Output,
) -> Result<()> {
    let me: Whoami = client.get("/v1/me", TIMEOUT).await?;
    let (user, _) = target(&me, user)?;
    let path = format!("/v1/users/{user}/sessions");
    match command {
        SessionCommand::List => {
            let sessions: Vec<SessionSummary> = client.get(&path, TIMEOUT).await?;
            if out.json(&sessions)? {
                return Ok(());
            }
            if sessions.is_empty() {
                println!("{user} is not signed in anywhere");
                return Ok(());
            }
            let now = unix_now();
            let rows: Vec<Vec<String>> = sessions
                .into_iter()
                .map(|session| {
                    let id = session.id.to_string();
                    let id = if session.current {
                        format!("{id} (this one)")
                    } else {
                        id
                    };
                    vec![
                        id,
                        session.address.unwrap_or_default(),
                        session.agent.unwrap_or_default(),
                        ago(now.saturating_sub(session.last_used_at)),
                    ]
                })
                .collect();
            print!(
                "{}",
                render(&["SESSION", "ADDRESS", "CLIENT", "USED"], &rows)
            );
        }
        SessionCommand::Remove { id } => {
            client.delete(&format!("{path}/{id}"), TIMEOUT).await?;
            println!("{} signed out session {id}", style::out(Paint::Green, "✓"));
        }
    }
    Ok(())
}
