use std::time::Duration;

use anyhow::Result;
use bird_core::Name;

use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Spinner, prompt};

const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) async fn run(client: &ApiClient, name: &Name, purge: bool, yes: bool) -> Result<()> {
    let path = if purge {
        prompt::confirm_name(
            &format!(
                "this removes {name} and deletes its volumes with all their data; backups are kept"
            ),
            name,
            yes,
        )?;
        format!("/v1/services/{name}?purge=true")
    } else {
        prompt::confirm(
            &format!("remove {name} and destroy its machines? its volumes are kept"),
            yes,
        )?;
        format!("/v1/services/{name}")
    };
    let spinner = Spinner::start(format!("removing {name}"));
    client.delete(&path, TIMEOUT).await?;
    drop(spinner);
    let done = if purge {
        format!("removed {name} and deleted its volumes")
    } else {
        format!("removed {name}")
    };
    println!("{} {done}", style::out(Paint::Green, "✓"));
    Ok(())
}
