use std::collections::HashSet;
use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use bird_api::{LogEntry, LogStream, ServiceSummary};
use bird_core::{MachineId, MachineState, Name};

use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);
const MACHINE_LABEL_LEN: usize = 6;

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    tail: u32,
    follow: bool,
    out: Output,
) -> Result<()> {
    let path = format!("/v1/services/{name}/logs?tail={tail}");
    if follow {
        let mut labels = Labels::new(running_machines(client, name).await? > 1);
        return client
            .stream_lines(&format!("{path}&follow=true"), TIMEOUT, |line| {
                if out.json {
                    println!("{line}");
                    return Ok(());
                }
                let entry: LogEntry = serde_json::from_str(line)?;
                print(&entry, labels.needed(entry.machine))
            })
            .await;
    }
    let entries: Vec<LogEntry> = client.get(&path, TIMEOUT).await?;
    if out.json {
        for entry in &entries {
            println!("{}", serde_json::to_string(entry)?);
        }
        return Ok(());
    }
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
    let service: ServiceSummary = client.get(&format!("/v1/services/{name}"), TIMEOUT).await?;
    Ok(service
        .deployment
        .iter()
        .flat_map(|d| &d.machines)
        .filter(|m| m.state == MachineState::Running)
        .count())
}

// lines are labeled once a second machine shows up, such as a replacement during a deploy
struct Labels {
    first: Option<MachineId>,
    on: bool,
}

impl Labels {
    const fn new(on: bool) -> Self {
        Self { first: None, on }
    }

    fn needed(&mut self, machine: MachineId) -> bool {
        let first = *self.first.get_or_insert(machine);
        self.on |= first != machine;
        self.on
    }
}

fn print(entry: &LogEntry, labeled: bool) -> Result<()> {
    let prefix = if labeled {
        format!("{} ", style::out(Paint::Cyan, label(entry.machine)))
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
pub(super) fn label(machine: MachineId) -> String {
    let id = machine.to_string();
    id.get(id.len().saturating_sub(MACHINE_LABEL_LEN)..)
        .unwrap_or(&id)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_start_with_a_second_machine() {
        let one: MachineId = "01a103aa-d2e9-70f0-b516-f428629a3236".parse().unwrap();
        let two: MachineId = "01a103aa-d2e9-70f0-b516-f428629a3237".parse().unwrap();
        let mut labels = Labels::new(false);
        assert!(!labels.needed(one));
        assert!(!labels.needed(one));
        assert!(labels.needed(two));
        assert!(labels.needed(one));
        assert!(Labels::new(true).needed(one));
    }

    #[test]
    fn labels_use_random_tail() {
        let machine: MachineId = "01a103aa-d2e9-70f0-b516-f428629a3236".parse().unwrap();
        assert_eq!(label(machine), "9a3236");
    }
}
