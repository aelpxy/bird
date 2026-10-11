use bird_core::{DeploymentId, ServiceId};
use rusqlite::params;

use crate::{Result, Store};

// enough history to compare recent deploys without letting logs grow forever
const KEPT_PER_SERVICE: u32 = 20;

impl Store {
    /// Saves a finished deploy's log, encoded by the caller, and drops logs of the service's
    /// older deployments
    pub fn put_deployment_log(
        &self,
        service_id: ServiceId,
        deployment_id: DeploymentId,
        lines: &str,
    ) -> Result<()> {
        self.execute(
            "INSERT INTO deployment_logs (deployment_id, lines) VALUES (?1, ?2)
             ON CONFLICT (deployment_id) DO UPDATE SET lines = excluded.lines",
            params![deployment_id.to_string(), lines],
        )?;
        self.execute(
            "DELETE FROM deployment_logs WHERE deployment_id IN (
                 SELECT id FROM deployments WHERE service_id = ?1
                 ORDER BY created_at DESC, id DESC LIMIT -1 OFFSET ?2)",
            params![service_id.to_string(), KEPT_PER_SERVICE],
        )?;
        Ok(())
    }

    pub fn deployment_log(&self, deployment_id: DeploymentId) -> Result<Option<String>> {
        self.query_one(
            "SELECT lines FROM deployment_logs WHERE deployment_id = ?1",
            [deployment_id.to_string()],
            |row| row.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::setup;

    #[test]
    fn keeps_the_recent_deployments_logs() {
        let (store, service) = setup();
        let mut deployments = Vec::new();
        for n in 0..=super::KEPT_PER_SERVICE {
            let deployment = store.create_deployment(&service).unwrap();
            store
                .put_deployment_log(service.id, deployment.id, &format!("[{n}]"))
                .unwrap();
            deployments.push(deployment.id);
        }
        let newest = *deployments.last().unwrap();
        assert_eq!(
            store.deployment_log(newest).unwrap().as_deref(),
            Some(format!("[{}]", super::KEPT_PER_SERVICE).as_str())
        );
        assert_eq!(store.deployment_log(deployments[0]).unwrap(), None);
        assert!(store.deployment_log(deployments[1]).unwrap().is_some());
    }
}
