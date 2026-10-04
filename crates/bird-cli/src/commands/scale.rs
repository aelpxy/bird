use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{ScaleRequest, ServiceSummary};
use bird_core::{MachineState, Name, Replicas, ServiceState};

use crate::client::ApiClient;
use crate::ui::Spinner;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_secs(1);
const CONVERGE_TIMEOUT: Duration = Duration::from_mins(5);

pub(crate) async fn run(client: &ApiClient, name: &Name, replicas: Replicas) -> Result<()> {
    client
        .put(
            &client.scoped(&format!("services/{name}/scale")),
            &ScaleRequest { replicas },
            TIMEOUT,
        )
        .await?;
    let spinner = Spinner::start(format!("scaling {name} to {replicas} machines"));
    let reached = converge(client, name, &spinner).await?;
    drop(spinner);
    match reached {
        Converged::Running(running) => println!(
            "{} {name} runs {running} {}",
            style::out(Paint::Green, "✓"),
            if running == 1 { "machine" } else { "machines" }
        ),
        Converged::NotDeployed => {
            println!("{name} is not deployed yet, it will start {replicas} machines on deploy");
        }
        Converged::Stopped => {
            println!("{name} is stopped, it will run {replicas} machines once started");
        }
    }
    Ok(())
}

pub(super) enum Converged {
    Running(usize),
    NotDeployed,
    Stopped,
}

// waits until exactly the wanted number of machines run, showing progress on the spinner
pub(super) async fn converge(
    client: &ApiClient,
    name: &Name,
    spinner: &Spinner,
) -> Result<Converged> {
    let deadline = tokio::time::Instant::now() + CONVERGE_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        let service: ServiceSummary = client
            .get(&client.scoped(&format!("services/{name}")), TIMEOUT)
            .await?;
        if service.state == ServiceState::Stopped {
            return Ok(Converged::Stopped);
        }
        let Some(deployment) = service.deployment else {
            return Ok(Converged::NotDeployed);
        };
        let running = deployment
            .machines
            .iter()
            .filter(|m| m.state == MachineState::Running)
            .count();
        let wanted = usize::from(service.replicas.get());
        if running == wanted && deployment.machines.len() == running {
            return Ok(Converged::Running(running));
        }
        spinner.set(format!("{name}: {running}/{wanted} running"));
        tokio::time::sleep(POLL).await;
    }
    bail!("{name} did not reach its machine count in time, check `bird logs -s {name}`")
}
