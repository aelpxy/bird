use std::time::Duration;

use anyhow::Result;
use bird_api::ServiceSummary;
use bird_core::MachineState;

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);
const HEADER: [&str; 5] = ["NAME", "IMAGE", "STATUS", "MACHINES", "DOMAINS"];

pub(crate) async fn run(client: &ApiClient) -> Result<()> {
    let services: Vec<ServiceSummary> = client.get("/v1/services", TIMEOUT).await?;
    if services.is_empty() {
        println!("no services yet, deploy one with `bird deploy <name> <image>`");
        return Ok(());
    }
    let rows: Vec<[String; 5]> = services.iter().map(row).collect();
    print!("{}", render(&rows));
    Ok(())
}

fn row(service: &ServiceSummary) -> [String; 5] {
    let (status, machines) = match &service.deployment {
        Some(deployment) => {
            let running = deployment
                .machines
                .iter()
                .filter(|m| m.state == MachineState::Running)
                .count();
            (
                deployment.status.to_string(),
                format!("{running}/{}", deployment.machines.len()),
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
    [
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

fn render(rows: &[[String; 5]]) -> String {
    let mut widths = HEADER.map(str::len);
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let mut out = String::new();
    let header = HEADER.map(str::to_owned);
    for row in std::iter::once(&header).chain(rows) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns() {
        let rows = [[
            "web".to_owned(),
            "nginx:alpine".to_owned(),
            "active".to_owned(),
            "1/1".to_owned(),
            "web.localhost".to_owned(),
        ]];
        let output = render(&rows);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "NAME  IMAGE         STATUS  MACHINES  DOMAINS");
        assert_eq!(
            lines[1],
            "web   nginx:alpine  active  1/1       web.localhost"
        );
    }
}
