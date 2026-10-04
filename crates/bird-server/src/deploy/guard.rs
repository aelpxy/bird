use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bird_core::{EnvironmentId, Name};

use crate::{Error, Result};

const WAIT_POLL: Duration = Duration::from_millis(200);

#[derive(Clone, Default)]
pub(crate) struct DeployGuard {
    busy: Arc<Mutex<HashSet<(EnvironmentId, Name)>>>,
}

pub(crate) struct Ticket {
    guard: DeployGuard,
    key: (EnvironmentId, Name),
}

impl DeployGuard {
    pub(crate) fn begin(&self, environment: EnvironmentId, name: &Name) -> Result<Ticket> {
        let key = (environment, name.clone());
        let mut busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        if !busy.insert(key.clone()) {
            return Err(Error::Busy(name.clone()));
        }
        Ok(Ticket {
            guard: self.clone(),
            key,
        })
    }

    // user requests queue behind a running operation, such as the supervisor replacing a machine
    pub(crate) async fn wait_for(
        &self,
        environment: EnvironmentId,
        name: &Name,
        patience: Duration,
    ) -> Result<Ticket> {
        let deadline = tokio::time::Instant::now() + patience;
        loop {
            match self.begin(environment, name) {
                Err(Error::Busy(_)) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(WAIT_POLL).await;
                }
                result => return result,
            }
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.guard
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn waits_until_the_running_operation_ends() {
        let guard = DeployGuard::default();
        let web: Name = "web".parse().unwrap();
        let env = EnvironmentId::generate();
        let ticket = guard.begin(env, &web).unwrap();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            drop(ticket);
        });
        assert!(
            guard
                .wait_for(env, &web, Duration::from_secs(2))
                .await
                .is_ok()
        );

        let _held = guard.begin(env, &web).unwrap();
        assert!(matches!(
            guard.wait_for(env, &web, Duration::from_millis(300)).await,
            Err(Error::Busy(_))
        ));
    }

    #[test]
    fn allows_one_operation_per_service() {
        let guard = DeployGuard::default();
        let web: Name = "web".parse().unwrap();
        let api: Name = "api".parse().unwrap();
        let (production, staging) = (EnvironmentId::generate(), EnvironmentId::generate());

        let ticket = guard.begin(production, &web).unwrap();
        assert!(matches!(guard.begin(production, &web), Err(Error::Busy(_))));
        let _other = guard.begin(production, &api).unwrap();
        let _same_name_elsewhere = guard.begin(staging, &web).unwrap();

        drop(ticket);
        assert!(guard.begin(production, &web).is_ok());
    }
}
