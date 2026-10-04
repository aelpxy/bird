use std::fmt::Write;
use std::time::Duration;

use anyhow::Result;
use bird_api::ServiceSummary;
use bird_core::{MachineState, Name, ServiceState};

use super::history::{ago, unix_now};
use super::logs::label;
use super::table::render;
use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
const LABEL_WIDTH: usize = 11;

pub(crate) async fn run(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let service: ServiceSummary = client.get(&format!("/v1/services/{name}"), TIMEOUT).await?;
    if out.json(&service)? {
        return Ok(());
    }
    print!("{}", describe(&service, unix_now()));
    Ok(())
}

fn describe(service: &ServiceSummary, now: i64) -> String {
    let mut text = String::new();
    let headline = match &service.deployment {
        _ if service.state == ServiceState::Stopped => style::out(Paint::Dim, "stopped"),
        Some(deployment) => style::out(style::deployment(deployment.status), deployment.status),
        None => style::out(Paint::Dim, "not deployed"),
    };
    let _ = writeln!(
        text,
        "{}  {headline}",
        style::out(Paint::Bold, &service.name)
    );
    let mut field = |label: &str, value: String| {
        let label = format!("{label:<LABEL_WIDTH$}");
        let _ = writeln!(text, "  {}{value}", style::out(Paint::Dim, label));
    };
    field("image", service.image.to_string());
    if let Some(deployment) = &service.deployment {
        let age = ago(now.saturating_sub(deployment.created_at));
        field(
            "deployment",
            format!("{} {}", deployment.id, style::out(Paint::Dim, age)),
        );
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
        field(
            "machines",
            style::out(paint, format!("{running}/{} running", service.replicas)),
        );
    }
    let domains: Vec<String> = service.domains.iter().map(ToString::to_string).collect();
    field(
        "domains",
        if domains.is_empty() {
            style::out(Paint::Dim, "none")
        } else {
            domains.join(", ")
        },
    );
    field(
        "internal",
        format!("{}.internal:{}", service.name, service.port),
    );
    field(
        "health",
        format!(
            "{} {}",
            service.health,
            style::out(Paint::Dim, format!("({} to start)", service.health_timeout))
        ),
    );
    field(
        "resources",
        format!("{} cpu, {} memory", service.cpus, service.memory),
    );
    for volume in &service.volumes {
        field("volume", format!("{} → {}", volume.name, volume.path));
    }
    if let Some(schedule) = service.backup_schedule {
        field(
            "backups",
            format!("every {}, keep {}", schedule.every, schedule.keep),
        );
    } else if !service.volumes.is_empty() {
        field("backups", style::out(Paint::Dim, "not scheduled"));
    }

    let machines = service
        .deployment
        .iter()
        .flat_map(|d| &d.machines)
        .map(|m| {
            vec![
                label(m.id),
                style::out(style::machine(m.state), m.state),
                ago(now.saturating_sub(m.updated_at)),
            ]
        })
        .collect::<Vec<_>>();
    if !machines.is_empty() {
        text.push('\n');
        text.push_str(&render(&["MACHINE", "STATE", "SINCE"], &machines));
    }
    text
}

#[cfg(test)]
mod tests {
    use bird_api::{DeploymentSummary, MachineSummary};
    use bird_core::{DeploymentStatus, HealthCheck};

    use super::*;

    #[test]
    fn describes_a_running_service() {
        let service = ServiceSummary {
            name: "web".parse().unwrap(),
            state: ServiceState::Running,
            image: "nginx:alpine".parse().unwrap(),
            port: bird_core::Port::HTTP,
            replicas: bird_core::Replicas::ONE,
            memory: bird_core::MemoryLimit::DEFAULT,
            cpus: bird_core::CpuLimit::DEFAULT,
            health: HealthCheck::Http,
            health_timeout: bird_core::HealthTimeout::DEFAULT,
            domains: vec!["web.localhost".parse().unwrap()],
            volumes: Vec::new(),
            backup_schedule: None,
            deployment: Some(DeploymentSummary {
                id: "01a10467-92a5-7040-8b88-e1572362b0c3".parse().unwrap(),
                status: DeploymentStatus::Active,
                created_at: 1000,
                machines: vec![MachineSummary {
                    id: "01a10467-9f40-7541-b34b-b7cfd4a516d4".parse().unwrap(),
                    state: MachineState::Running,
                    updated_at: 1100,
                }],
            }),
        };
        let text = describe(&service, 1160);
        assert!(text.starts_with("web  active\n"), "{text}");
        assert!(text.contains("  machines   1/1 running\n"), "{text}");
        assert!(text.contains("  internal   web.internal:80\n"), "{text}");
        assert!(text.contains("  health     http (1m to start)\n"), "{text}");
        assert!(text.contains("a516d4   running  1m ago"), "{text}");
    }
}
