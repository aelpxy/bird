use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::check;
use crate::query::encode;
use crate::{Podman, Result};

const NANOS_PER_MILLICORE: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerStats {
    pub cpu_nanos: u64,
    pub sampled_at_nanos: u64,
    pub memory_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub processes: u64,
}

impl ContainerStats {
    // podman's own cpu figure is the average since the container started, not current load
    #[must_use]
    pub fn cpu_millicores_since(&self, earlier: &Self) -> Option<u64> {
        let used = self.cpu_nanos.checked_sub(earlier.cpu_nanos)?;
        let elapsed = self
            .sampled_at_nanos
            .checked_sub(earlier.sampled_at_nanos)?;
        let per_millicore = elapsed.checked_div(NANOS_PER_MILLICORE)?;
        used.checked_div(per_millicore)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatsResponse {
    #[serde(default)]
    stats: Option<Vec<Entry>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Entry {
    #[serde(rename = "ContainerID")]
    container_id: String,
    #[serde(rename = "CPUNano")]
    cpu_nano: u64,
    system_nano: u64,
    mem_usage: u64,
    #[serde(default)]
    network: Option<BTreeMap<String, Interface>>,
    #[serde(rename = "PIDs")]
    pids: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Interface {
    rx_bytes: u64,
    tx_bytes: u64,
}

impl From<Entry> for ContainerStats {
    fn from(entry: Entry) -> Self {
        let interfaces = entry.network.unwrap_or_default();
        Self {
            cpu_nanos: entry.cpu_nano,
            sampled_at_nanos: entry.system_nano,
            memory_bytes: entry.mem_usage,
            net_rx_bytes: interfaces.values().map(|i| i.rx_bytes).sum(),
            net_tx_bytes: interfaces.values().map(|i| i.tx_bytes).sum(),
            processes: entry.pids,
        }
    }
}

impl Podman {
    // one snapshot per container keyed by full id; fails whole when any of them is gone
    pub async fn stats(&self, ids: &[&str]) -> Result<BTreeMap<String, ContainerStats>> {
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut path = "/containers/stats?stream=false".to_owned();
        for id in ids {
            path.push_str("&containers=");
            path.push_str(&encode(id));
        }
        let response = self.get(&path).await?;
        let body = check(response, || "container stats".to_owned())?;
        Ok(parse(&body)?)
    }
}

fn parse(body: &[u8]) -> serde_json::Result<BTreeMap<String, ContainerStats>> {
    let response: StatsResponse = serde_json::from_slice(body)?;
    Ok(response
        .stats
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.container_id.clone(), entry.into()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stats() {
        let raw = br#"{"Error": null, "Stats": [{
            "AvgCPU": 0.0015, "ContainerID": "8b2e", "Name": "bird-app", "PerCPU": null,
            "CPU": 0.0015, "CPUNano": 358079000, "CPUSystemNano": 252441,
            "SystemNano": 1791084069025095554, "MemUsage": 13873152, "MemLimit": 1073741824,
            "MemPerc": 1.29, "Network": {
                "eth0": {"RxBytes": 131698, "RxDropped": 0, "RxErrors": 0, "RxPackets": 1745,
                         "TxBytes": 1608, "TxDropped": 0, "TxErrors": 0, "TxPackets": 22},
                "eth1": {"RxBytes": 2, "TxBytes": 3}
            },
            "BlockInput": 0, "BlockOutput": 0, "PIDs": 17, "UpTime": 358079000, "Duration": 358079000
        }, {
            "ContainerID": "a885", "CPUNano": 0, "SystemNano": 0, "MemUsage": 0, "Network": null, "PIDs": 0
        }]}"#;
        let stats = parse(raw).unwrap();
        let app = stats["8b2e"];
        assert_eq!(app.memory_bytes, 13_873_152);
        assert_eq!(app.net_rx_bytes, 131_700);
        assert_eq!(app.net_tx_bytes, 1611);
        assert_eq!(app.processes, 17);
        assert_eq!(stats["a885"].net_rx_bytes, 0);
    }

    #[test]
    fn measures_cpu_between_two_samples() {
        let sample = |cpu_nanos, sampled_at_nanos| ContainerStats {
            cpu_nanos,
            sampled_at_nanos,
            memory_bytes: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            processes: 0,
        };
        let busy = sample(4_055_510_000, 1_011_123_807);
        let before = sample(3_047_386_000, 0);
        assert_eq!(busy.cpu_millicores_since(&before), Some(997));
        let idle = sample(3_047_386_000, 500_000_000);
        assert_eq!(idle.cpu_millicores_since(&before), Some(0));
        assert_eq!(before.cpu_millicores_since(&busy), None);
        assert_eq!(before.cpu_millicores_since(&before), None);
    }
}
