use std::time::Duration;

use anyhow::Result;
use bird_api::{DeployRequest, DeployResponse};

use crate::args::DeployArgs;
use crate::client::ApiClient;

// covers a slow image pull plus the app's startup window on the server
const DEPLOY_TIMEOUT: Duration = Duration::from_mins(15);

pub(crate) async fn run(client: &ApiClient, args: DeployArgs) -> Result<()> {
    println!("deploying {} ({})...", args.name, args.image);
    let request = DeployRequest {
        name: args.name,
        image: args.image,
        port: args.port,
        domain: args.domain,
        env: args.env.into_iter().collect(),
    };
    let response: DeployResponse = client.post("/v1/deploy", &request, DEPLOY_TIMEOUT).await?;
    println!(
        "deployed {} (deployment {})",
        response.service, response.deployment_id
    );
    for domain in &response.domains {
        println!("  -> {domain}");
    }
    Ok(())
}
