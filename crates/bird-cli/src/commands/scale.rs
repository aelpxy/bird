use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{ScaleRequest, ServiceSummary};
use bird_core::{MachineState, Name, Replicas};

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_secs(1);
const CONVERGE_TIMEOUT: Duration = Duration::from_mins(5);

pub(crate) async fn run(client: &ApiClient, name: &Name, replicas: Replicas) -> Result<()> {
    client
        .put(
            &format!("/v1/services/{name}/scale"),
            &ScaleRequest { replicas },
            TIMEOUT,
        )
        .await?;
    println!("scaling {name} to {replicas} machines...");

    let deadline = tokio::time::Instant::now() + CONVERGE_TIMEOUT;
    let mut last = None;
    while tokio::time::Instant::now() < deadline {
        let services: Vec<ServiceSummary> = client.get("/v1/services", TIMEOUT).await?;
        let Some(service) = services.into_iter().find(|s| &s.name == name) else {
            bail!("service {name} disappeared while scaling");
        };
        let Some(deployment) = service.deployment else {
            println!("{name} is not deployed yet, it will start {replicas} machines on deploy");
            return Ok(());
        };
        let running = deployment
            .machines
            .iter()
            .filter(|m| m.state == MachineState::Running)
            .count();
        let total = deployment.machines.len();
        if last != Some(running) {
            println!("  {running}/{replicas} running");
            last = Some(running);
        }
        if running == usize::from(replicas.get()) && total == running {
            println!("{name} is running {replicas} machines");
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
    bail!("{name} did not reach {replicas} machines in time, check `bird logs {name}`")
}
