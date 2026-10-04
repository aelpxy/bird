use std::time::Duration;

use anyhow::Result;
use bird_api::ServiceSummary;
use bird_core::{MachineState, ServiceState};

use super::table::render;
use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
const HEADER: [&str; 5] = ["NAME", "STATUS", "MACHINES", "IMAGE", "DOMAINS"];

pub(crate) async fn run(client: &ApiClient, out: Output) -> Result<()> {
    let services: Vec<ServiceSummary> = client.get("/v1/services", TIMEOUT).await?;
    if out.json(&services)? {
        return Ok(());
    }
    if services.is_empty() {
        println!(
            "no services yet, start with {} in your app's directory",
            style::out(Paint::Bold, "bird init")
        );
        return Ok(());
    }
    let rows: Vec<Vec<String>> = services.iter().map(row).collect();
    print!("{}", render(&HEADER, &rows));
    Ok(())
}

fn row(service: &ServiceSummary) -> Vec<String> {
    let (status, machines) = match &service.deployment {
        _ if service.state == ServiceState::Stopped => (
            style::out(Paint::Dim, "stopped"),
            style::out(Paint::Dim, format!("0/{}", service.replicas)),
        ),
        Some(deployment) => {
            let running = deployment
                .machines
                .iter()
                .filter(|m| m.state == MachineState::Running)
                .count();
            let paint = if running == usize::from(service.replicas.get()) {
                Paint::Green
            } else {
                Paint::Yellow
            };
            (
                style::out(style::deployment(deployment.status), deployment.status),
                style::out(paint, format!("{running}/{}", service.replicas)),
            )
        }
        None if service.failed_deploy.is_some() => (
            style::out(Paint::Red, "failed"),
            style::out(Paint::Red, format!("0/{}", service.replicas)),
        ),
        None => (
            style::out(Paint::Dim, "not deployed"),
            style::out(Paint::Dim, "0/0"),
        ),
    };
    let domains = service
        .domains
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        style::out(Paint::Bold, &service.name),
        status,
        machines,
        service.image.to_string(),
        if domains.is_empty() {
            style::out(Paint::Dim, "-")
        } else {
            domains
        },
    ]
}
