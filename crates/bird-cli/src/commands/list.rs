use std::time::Duration;

use anyhow::Result;
use bird_api::ServiceSummary;
use bird_core::MachineState;

use super::table::render;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);
const HEADER: [&str; 5] = ["NAME", "IMAGE", "STATUS", "MACHINES", "DOMAINS"];

pub(crate) async fn run(client: &ApiClient) -> Result<()> {
    let services: Vec<ServiceSummary> = client.get("/v1/services", TIMEOUT).await?;
    if services.is_empty() {
        println!("no services yet, deploy one with `bird deploy <name> <image>`");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = services.iter().map(row).collect();
    print!("{}", render(&HEADER, &rows));
    Ok(())
}

fn row(service: &ServiceSummary) -> Vec<String> {
    let (status, machines) = match &service.deployment {
        Some(deployment) => {
            let running = deployment
                .machines
                .iter()
                .filter(|m| m.state == MachineState::Running)
                .count();
            (
                deployment.status.to_string(),
                format!("{running}/{}", service.replicas),
            )
        }
        None => ("not deployed".to_owned(), "0/0".to_owned()),
    };
    let domains = service
        .domains
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        service.name.to_string(),
        service.image.to_string(),
        status,
        machines,
        if domains.is_empty() {
            "-".to_owned()
        } else {
            domains
        },
    ]
}
