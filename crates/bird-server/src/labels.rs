use std::collections::BTreeMap;

use bird_core::{DeploymentId, EnvironmentId, MachineId, Service, Volume};

pub(crate) const MACHINE: &str = "bird.machine";
const ENVIRONMENT: &str = "bird.environment-id";

// other services reach this one as `<name>` or `<name>.internal` on the private network
pub(crate) fn aliases(service: &Service) -> Vec<String> {
    vec![
        service.name.to_string(),
        format!("{}.internal", service.name),
    ]
}

pub(crate) fn environment_filter(environment: EnvironmentId) -> String {
    format!("{ENVIRONMENT}={environment}")
}

pub(crate) fn for_volume(volume: &Volume) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("bird.managed".to_owned(), "true".to_owned()),
        ("bird.volume".to_owned(), volume.id.to_string()),
    ])
}

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
        (ENVIRONMENT.to_owned(), service.environment_id.to_string()),
        (MACHINE.to_owned(), machine.to_string()),
    ])
}
