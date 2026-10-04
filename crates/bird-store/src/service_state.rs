use bird_core::{ServiceId, ServiceState};
use rusqlite::params;

use crate::error::expect_changed;
use crate::{Result, Store};

impl Store {
    pub fn set_service_state(&self, id: ServiceId, state: ServiceState) -> Result<()> {
        let changed = self.execute(
            "UPDATE services SET state = ?2 WHERE id = ?1",
            params![id.to_string(), state.as_str()],
        )?;
        expect_changed(changed, "service")
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{ServiceId, ServiceState};

    use crate::Error;
    use crate::testing::setup;

    #[test]
    fn stores_service_state() {
        let (store, service) = setup();
        assert_eq!(service.state, ServiceState::Running);
        store
            .set_service_state(service.id, ServiceState::Stopped)
            .unwrap();
        assert_eq!(
            store.service(service.id).unwrap().unwrap().state,
            ServiceState::Stopped
        );
        assert!(matches!(
            store.set_service_state(ServiceId::generate(), ServiceState::Stopped),
            Err(Error::NotFound("service"))
        ));
    }
}
