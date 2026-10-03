use std::collections::BTreeMap;

use bird_core::{DeploymentId, MachineId, Service};

pub(crate) fn for_machine(
    service: &Service,
    deployment: DeploymentId,
    machine: MachineId,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("bird.managed".to_owned(), "true".to_owned()),
        ("bird.service".to_owned(), service.name.to_string()),
        ("bird.service-id".to_owned(), service.id.to_string()),
        ("bird.deployment".to_owned(), deployment.to_string()),
        ("bird.machine".to_owned(), machine.to_string()),
    ])
}
