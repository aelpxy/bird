use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{ScaleRequest, ServiceSummary};
use bird_core::{MachineState, Name, Replicas};

use crate::client::ApiClient;
use crate::ui::Spinner;
use crate::ui::style::{self, Paint};

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
    let spinner = Spinner::start(format!("scaling {name} to {replicas} machines"));

    let deadline = tokio::time::Instant::now() + CONVERGE_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        let service: ServiceSummary = client.get(&format!("/v1/services/{name}"), TIMEOUT).await?;
        let Some(deployment) = service.deployment else {
            drop(spinner);
            println!("{name} is not deployed yet, it will start {replicas} machines on deploy");
            return Ok(());
        };
        let running = deployment
            .machines
            .iter()
            .filter(|m| m.state == MachineState::Running)
            .count();
        if running == usize::from(replicas.get()) && deployment.machines.len() == running {
            drop(spinner);
            println!(
                "{} {name} runs {replicas} {}",
                style::out(Paint::Green, "✓"),
                if replicas == Replicas::ONE {
                    "machine"
                } else {
                    "machines"
                }
            );
            return Ok(());
        }
        spinner.set(format!("scaling {name}: {running}/{replicas} running"));
        tokio::time::sleep(POLL).await;
    }
    bail!("{name} did not reach {replicas} machines in time, check `bird logs -s {name}`")
}
