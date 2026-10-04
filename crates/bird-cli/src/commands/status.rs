use std::fmt::Write;
use std::time::Duration;

use anyhow::Result;
use bird_api::{MachineStats, MachineSummary, ServiceSummary};
use bird_core::{MachineState, Name, ServiceState};

use super::backup::size;
use super::history::{ago, unix_now};
use super::logs::label;
use super::table::render;
use super::watch;
use crate::client::ApiClient;
use crate::scope::Scope;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
const LABEL_WIDTH: usize = 11;

pub(crate) async fn run(client: &ApiClient, name: &Name, watch: bool, out: Output) -> Result<()> {
    if watch {
        return watch::run(client, name, out).await;
    }
    let service = fetch(client, name).await?;
    if out.json(&service)? {
        return Ok(());
    }
    print!("{}", describe(&service, client.scope(), unix_now()));
    Ok(())
}

pub(super) async fn fetch(client: &ApiClient, name: &Name) -> Result<ServiceSummary> {
    client
        .get(
            &client.scoped(&format!("services/{name}?stats=true")),
            TIMEOUT,
        )
        .await
}

pub(super) fn describe(service: &ServiceSummary, scope: &Scope, now: i64) -> String {
    let mut text = String::new();
    let headline = match &service.deployment {
        _ if service.state == ServiceState::Stopped => style::out(Paint::Dim, "stopped"),
        Some(deployment) => style::out(style::deployment(deployment.status), deployment.status),
        None if service.failed_deploy.is_some() => style::out(Paint::Red, "failed"),
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
    field("project", scope.to_string());
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
    if let Some(failed) = service.failed_deploy {
        let age = ago(now.saturating_sub(failed.created_at));
        field(
            "failed",
            format!(
                "{} {}",
                style::out(Paint::Red, failed.id),
                style::out(Paint::Dim, format!("{age}, see `bird history`"))
            ),
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

    text.push_str(&machine_table(service, now));
    text
}

fn machine_table(service: &ServiceSummary, now: i64) -> String {
    let machines: Vec<&MachineSummary> = service
        .deployment
        .iter()
        .flat_map(|d| &d.machines)
        .collect();
    let sampled = machines.iter().any(|m| m.stats.is_some());
    let rows = machines
        .iter()
        .map(|m| {
            let mut row = vec![
                label(m.id),
                style::out(style::machine(m.state), m.state),
                ago(now.saturating_sub(m.updated_at)),
            ];
            if sampled {
                row.extend(usage(service, m.stats.as_ref()));
            }
            row
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return String::new();
    }
    let mut header = vec!["MACHINE", "STATE", "SINCE"];
    if sampled {
        header.extend(["CPU", "MEMORY", "NET"]);
    }
    format!("\n{}", render(&header, &rows))
}

// cpu and memory as a share of the service's limits, since that is what runs out
fn usage(service: &ServiceSummary, stats: Option<&MachineStats>) -> [String; 3] {
    let Some(stats) = stats else {
        return ["-", "-", "-"].map(|cell| style::out(Paint::Dim, cell));
    };
    let cpu = percent(stats.cpu_millicores, u64::from(service.cpus.millicores()));
    let memory = percent(stats.memory_bytes, service.memory.bytes());
    [
        format!("{cpu}%"),
        format!(
            "{} {}",
            size(stats.memory_bytes),
            style::out(Paint::Dim, format!("{memory}%"))
        ),
        format!(
            "↓{} ↑{}",
            size(stats.net_rx_bytes),
            size(stats.net_tx_bytes)
        ),
    ]
}

fn percent(used: u64, limit: u64) -> u64 {
    used.saturating_mul(100).checked_div(limit).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use bird_api::DeploymentSummary;
    use bird_core::{DeploymentStatus, HealthCheck};

    use super::*;

    fn service() -> ServiceSummary {
        ServiceSummary {
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
                    stats: None,
                }],
            }),
            failed_deploy: None,
        }
    }

    #[test]
    fn describes_a_running_service() {
        let text = describe(&service(), &Scope::fallback(), 1160);
        assert!(text.starts_with("web  active\n"), "{text}");
        assert!(text.contains("  machines   1/1 running\n"), "{text}");
        assert!(text.contains("  internal   web.internal:80\n"), "{text}");
        assert!(text.contains("  health     http (1m to start)\n"), "{text}");
        assert!(text.contains("a516d4   running  1m ago"), "{text}");
        assert!(!text.contains("CPU"), "{text}");
        assert!(!text.contains("failed"), "{text}");
    }

    #[test]
    fn shows_a_failed_deploy() {
        let mut service = service();
        let failed = bird_api::FailedDeploy {
            id: "01a10467-92a5-7040-8b88-e1572362b0c4".parse().unwrap(),
            created_at: 1100,
        };
        service.failed_deploy = Some(failed);
        let text = describe(&service, &Scope::fallback(), 1160);
        assert!(text.starts_with("web  active\n"), "{text}");
        assert!(
            text.contains(
                "  failed     01a10467-92a5-7040-8b88-e1572362b0c4 1m ago, see `bird history`\n"
            ),
            "{text}"
        );
        service.deployment = None;
        assert!(describe(&service, &Scope::fallback(), 1160).starts_with("web  failed\n"));
    }

    #[test]
    fn shows_usage_against_the_limits() {
        let mut service = service();
        let deployment = service.deployment.as_mut().unwrap();
        deployment.machines[0].stats = Some(MachineStats {
            cpu_millicores: 250,
            memory_bytes: 128 * 1024 * 1024,
            net_rx_bytes: 1536,
            net_tx_bytes: 100,
            processes: 3,
        });
        let mut stopped = deployment.machines[0].clone();
        stopped.state = MachineState::Stopped;
        stopped.stats = None;
        deployment.machines.push(stopped);
        let text = describe(&service, &Scope::fallback(), 1160);
        assert!(
            text.contains("MACHINE  STATE    SINCE   CPU  MEMORY"),
            "{text}"
        );
        assert!(
            text.contains("running  1m ago  25%  128.0 MiB 12%  ↓1.5 KiB ↑100 B"),
            "{text}"
        );
        assert!(text.contains("stopped  1m ago  -    -"), "{text}");
    }
}
