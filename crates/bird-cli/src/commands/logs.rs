use std::collections::HashSet;
use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use bird_api::{LogEntry, LogStream, ServiceSummary};
use bird_core::{MachineId, MachineState, Name};

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);
const MACHINE_LABEL_LEN: usize = 6;

pub(crate) async fn run(client: &ApiClient, name: &Name, tail: u32, follow: bool) -> Result<()> {
    let path = format!("/v1/services/{name}/logs?tail={tail}");
    if follow {
        let labeled = running_machines(client, name).await? > 1;
        return client
            .stream_lines(&format!("{path}&follow=true"), TIMEOUT, |line| {
                print(&serde_json::from_str(line)?, labeled)
            })
            .await;
    }
    let entries: Vec<LogEntry> = client.get(&path, TIMEOUT).await?;
    let labeled = entries
        .iter()
        .map(|e| e.machine)
        .collect::<HashSet<_>>()
        .len()
        > 1;
    for entry in &entries {
        print(entry, labeled)?;
    }
    Ok(())
}

async fn running_machines(client: &ApiClient, name: &Name) -> Result<usize> {
    let services: Vec<ServiceSummary> = client.get("/v1/services", TIMEOUT).await?;
    Ok(services
        .iter()
        .filter(|s| &s.name == name)
        .filter_map(|s| s.deployment.as_ref())
        .flat_map(|d| &d.machines)
        .filter(|m| m.state == MachineState::Running)
        .count())
}

fn print(entry: &LogEntry, labeled: bool) -> Result<()> {
    let prefix = if labeled {
        format!("[{}] ", label(entry.machine))
    } else {
        String::new()
    };
    match entry.stream {
        LogStream::Stdout => writeln!(std::io::stdout(), "{prefix}{}", entry.text)?,
        LogStream::Stderr => writeln!(std::io::stderr(), "{prefix}{}", entry.text)?,
    }
    Ok(())
}

// the tail of a v7 uuid is random, its head is a timestamp shared by machines started together
fn label(machine: MachineId) -> String {
    let id = machine.to_string();
    id.get(id.len().saturating_sub(MACHINE_LABEL_LEN)..)
        .unwrap_or(&id)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_use_random_tail() {
        let machine: MachineId = "01a103aa-d2e9-70f0-b516-f428629a3236".parse().unwrap();
        assert_eq!(label(machine), "9a3236");
    }
}
