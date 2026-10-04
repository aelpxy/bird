use bird_core::{Name, Port, Service};

use crate::Store;

pub(crate) fn name(s: &str) -> Name {
    s.parse().unwrap()
}

pub(crate) fn setup() -> (Store, Service) {
    let store = Store::open_in_memory().unwrap();
    let org = store.create_org(&name("default")).unwrap();
    let project = store.create_project(org.id, &name("default")).unwrap();
    let env = store
        .create_environment(project.id, &name("production"), None)
        .unwrap();
    let service = store
        .create_service(
            env.id,
            &name("web"),
            &"nginx:latest".parse().unwrap(),
            Port::try_from(80).unwrap(),
        )
        .unwrap();
    (store, service)
}
