use bird_core::{Name, Port, Service};

use crate::Store;

pub(crate) fn name(s: &str) -> Name {
    s.parse().unwrap()
}

pub(crate) fn setup() -> (Store, Service) {
    let store = Store::open_in_memory().unwrap();
    let project = store.create_project(&name("default")).unwrap();
    let env = store
        .create_environment(project.id, &name("production"))
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
