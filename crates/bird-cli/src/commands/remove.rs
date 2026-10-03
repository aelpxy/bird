use std::time::Duration;

use anyhow::Result;
use bird_core::Name;

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) async fn run(client: &ApiClient, name: &Name, purge: bool) -> Result<()> {
    let path = if purge {
        format!("/v1/services/{name}?purge=true")
    } else {
        format!("/v1/services/{name}")
    };
    client.delete(&path, TIMEOUT).await?;
    if purge {
        println!("removed {name} and deleted its volumes");
    } else {
        println!("removed {name}");
    }
    Ok(())
}
