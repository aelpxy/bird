use std::time::Duration;

use anyhow::Result;
use bird_core::Name;

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) async fn run(client: &ApiClient, name: &Name) -> Result<()> {
    client
        .delete(&format!("/v1/services/{name}"), TIMEOUT)
        .await?;
    println!("removed {name}");
    Ok(())
}
