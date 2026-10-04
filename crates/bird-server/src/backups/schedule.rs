use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bird_core::{
    BackupKeep, BackupSchedule, BackupTrigger, EnvironmentId, Name, Service, ServiceId,
};
use tokio::time::{MissedTickBehavior, interval_at};

use super::create::snapshot;
use super::database::DatabaseBackups;
use crate::state::AppState;
use crate::{Error, Result};

const TICK: Duration = Duration::from_mins(1);
// a failed backup is tried again this much later instead of pausing the machines every minute
const RETRY_AFTER: Duration = Duration::from_mins(15);

pub(crate) async fn set(
    state: &AppState,
    environment: EnvironmentId,
    name: &Name,
    schedule: BackupSchedule,
) -> Result<()> {
    let service = state.service(environment, name).await?;
    let service_id = service.id;
    let has_volumes = state
        .db
        .call(move |store| {
            let has_volumes = !store.list_volumes(service_id)?.is_empty();
            if has_volumes {
                store.set_backup_schedule(service_id, schedule)?;
            }
            Ok(has_volumes)
        })
        .await?;
    if !has_volumes {
        return Err(Error::NothingToBackUp(name.clone()));
    }
    tracing::info!(service = %name, every = %schedule.every, keep = %schedule.keep, "backup schedule set");
    Ok(())
}

pub(crate) async fn clear(state: &AppState, environment: EnvironmentId, name: &Name) -> Result<()> {
    let service = state.service(environment, name).await?;
    let service_id = service.id;
    state
        .db
        .call(move |store| store.clear_backup_schedule(service_id))
        .await?;
    tracing::info!(service = %name, "backup schedule removed");
    Ok(())
}

pub(crate) struct Scheduler {
    state: AppState,
    database: Option<DatabaseBackups>,
    failed: HashMap<ServiceId, Instant>,
}

impl Scheduler {
    pub(crate) fn new(state: AppState, database: Option<DatabaseBackups>) -> Self {
        Self {
            state,
            database,
            failed: HashMap::new(),
        }
    }

    // a backup cut short by shutdown leaves its machines paused, which the supervisor undoes on start
    pub(crate) async fn run(mut self, shutdown: impl Future<Output = ()>) {
        let mut ticker = interval_at(tokio::time::Instant::now() + TICK, TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = self.tick() => {}
            }
        }
        tracing::info!("backup scheduler stopped");
    }

    async fn tick(&mut self) {
        if let Err(err) = self.back_up_services().await {
            tracing::warn!(error = %err, "could not run scheduled backups");
        }
        if let Some(database) = &mut self.database {
            database.back_up_if_due(&self.state).await;
        }
    }

    async fn back_up_services(&mut self) -> Result<()> {
        let schedules = self
            .state
            .db
            .call(|store| store.list_backup_schedules())
            .await?;
        self.failed
            .retain(|id, at| at.elapsed() < RETRY_AFTER && schedules.iter().any(|(s, _)| s == id));
        for (service_id, schedule) in schedules {
            if self.failed.contains_key(&service_id) {
                continue;
            }
            match self.back_up_if_due(service_id, schedule).await {
                Ok(()) => {}
                Err(err) => {
                    tracing::error!(service_id = %service_id, error = %err, "scheduled backup failed");
                    self.failed.insert(service_id, Instant::now());
                }
            }
        }
        Ok(())
    }

    async fn back_up_if_due(&self, service_id: ServiceId, schedule: BackupSchedule) -> Result<()> {
        let state = &self.state;
        let found = state
            .db
            .call(move |store| {
                let Some(service) = store.service(service_id)? else {
                    return Ok(None);
                };
                let last = store.last_backup_at(
                    service.environment_id,
                    &service.name,
                    BackupTrigger::Scheduled,
                )?;
                Ok(Some((service, last)))
            })
            .await?;
        let Some((service, last)) = found else {
            return Ok(());
        };
        if !is_due(last, schedule, unix_now()) {
            return Ok(());
        }
        // a deploy or another backup holds the ticket, the next tick tries again
        let Ok(ticket) = state.deploys.begin(service.environment_id, &service.name) else {
            return Ok(());
        };
        let backup = snapshot(state, &service, BackupTrigger::Scheduled).await?;
        drop(ticket);
        tracing::info!(service = %service.name, backup = %backup.id, "scheduled backup done");
        prune(state, &service, schedule.keep).await;
        Ok(())
    }
}

#[must_use]
fn is_due(last: Option<i64>, schedule: BackupSchedule, now: i64) -> bool {
    last.is_none_or(|at| now.saturating_sub(at) >= i64::from(schedule.every.secs()))
}

// only scheduled backups count toward keep; manual and restore backups are the operator's to delete
async fn prune(state: &AppState, service: &Service, keep: BackupKeep) {
    let (environment, name) = (service.environment_id, &service.name);
    let backups = match super::list(state, environment, name).await {
        Ok(backups) => backups,
        Err(err) => {
            tracing::warn!(service = %name, error = %err, "could not list backups to prune");
            return;
        }
    };
    let expired = backups
        .into_iter()
        .filter(|backup| backup.trigger == BackupTrigger::Scheduled)
        .skip(keep.count());
    for backup in expired {
        if let Err(err) = super::remove(state, environment, name, backup.id).await {
            tracing::warn!(service = %name, backup = %backup.id, error = %err, "could not delete an expired backup");
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_once_the_interval_passed() {
        let schedule = BackupSchedule {
            every: "1d".parse().unwrap(),
            keep: BackupKeep::WEEK,
        };
        assert!(is_due(None, schedule, 1000));
        assert!(!is_due(Some(1000), schedule, 1000 + 86_399));
        assert!(is_due(Some(1000), schedule, 1000 + 86_400));
        assert!(!is_due(Some(5000), schedule, 1000));
    }
}
