use bird_core::{BackupSchedule, BackupTrigger, EnvironmentId, Name, ServiceId};
use rusqlite::params;

use crate::{Result, Store, rows};

impl Store {
    pub fn set_backup_schedule(
        &self,
        service_id: ServiceId,
        schedule: BackupSchedule,
    ) -> Result<()> {
        self.execute(
            "INSERT INTO backup_schedules (service_id, every_secs, keep) VALUES (?1, ?2, ?3)
             ON CONFLICT (service_id) DO UPDATE SET every_secs = ?2, keep = ?3",
            params![
                service_id.to_string(),
                schedule.every.secs(),
                u16::from(schedule.keep)
            ],
        )
        .map_err(crate::error::write_error("backup schedule"))?;
        Ok(())
    }

    // removing a schedule that is not there succeeds
    pub fn clear_backup_schedule(&self, service_id: ServiceId) -> Result<()> {
        self.execute(
            "DELETE FROM backup_schedules WHERE service_id = ?1",
            [service_id.to_string()],
        )?;
        Ok(())
    }

    pub fn backup_schedule(&self, service_id: ServiceId) -> Result<Option<BackupSchedule>> {
        self.query_one(
            "SELECT every_secs, keep FROM backup_schedules WHERE service_id = ?1",
            [service_id.to_string()],
            rows::backup_schedule,
        )
    }

    pub fn list_backup_schedules(&self) -> Result<Vec<(ServiceId, BackupSchedule)>> {
        self.query_all(
            "SELECT every_secs, keep, service_id FROM backup_schedules ORDER BY service_id",
            [],
            rows::scheduled_service,
        )
    }

    pub fn last_backup_at(
        &self,
        environment_id: EnvironmentId,
        service: &Name,
        trigger: BackupTrigger,
    ) -> Result<Option<i64>> {
        let last: Option<Option<i64>> = self.query_one(
            "SELECT MAX(created_at) FROM backups
             WHERE environment_id = ?1 AND service_name = ?2 AND trigger = ?3",
            params![
                environment_id.to_string(),
                service.as_str(),
                trigger.as_str()
            ],
            |row| row.get(0),
        )?;
        Ok(last.flatten())
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{Backup, BackupId, BackupSchedule, BackupTrigger, ServiceId};

    use crate::Error;
    use crate::testing::setup;

    fn schedule(every: &str, keep: &str) -> BackupSchedule {
        BackupSchedule {
            every: every.parse().unwrap(),
            keep: keep.parse().unwrap(),
        }
    }

    #[test]
    fn sets_replaces_and_clears_schedules() {
        let (store, service) = setup();
        assert_eq!(store.backup_schedule(service.id).unwrap(), None);
        store
            .set_backup_schedule(service.id, schedule("1d", "7"))
            .unwrap();
        store
            .set_backup_schedule(service.id, schedule("6h", "28"))
            .unwrap();
        assert_eq!(
            store.backup_schedule(service.id).unwrap(),
            Some(schedule("6h", "28"))
        );
        assert_eq!(
            store.list_backup_schedules().unwrap(),
            vec![(service.id, schedule("6h", "28"))]
        );
        store.clear_backup_schedule(service.id).unwrap();
        store.clear_backup_schedule(service.id).unwrap();
        assert_eq!(store.list_backup_schedules().unwrap(), Vec::new());
        assert!(matches!(
            store.set_backup_schedule(ServiceId::generate(), schedule("1d", "7")),
            Err(Error::ParentNotFound("backup schedule"))
        ));
    }

    #[test]
    fn schedules_go_with_their_service() {
        let (store, service) = setup();
        store
            .set_backup_schedule(service.id, schedule("1d", "7"))
            .unwrap();
        store.delete_service(service.id).unwrap();
        assert_eq!(store.list_backup_schedules().unwrap(), Vec::new());
    }

    #[test]
    fn finds_the_last_backup_of_a_trigger() {
        let (mut store, service) = setup();
        let last = |store: &crate::Store, trigger| {
            store
                .last_backup_at(service.environment_id, &service.name, trigger)
                .unwrap()
        };
        assert_eq!(last(&store, BackupTrigger::Scheduled), None);
        let backup = |trigger| Backup {
            id: BackupId::generate(),
            environment_id: service.environment_id,
            service: service.name.clone(),
            trigger,
            storage: "local".to_owned(),
            volumes: Vec::new(),
            created_at: 0,
        };
        let manual = store.create_backup(backup(BackupTrigger::Manual)).unwrap();
        assert_eq!(last(&store, BackupTrigger::Scheduled), None);
        assert_eq!(last(&store, BackupTrigger::Manual), Some(manual.created_at));
        let scheduled = store
            .create_backup(backup(BackupTrigger::Scheduled))
            .unwrap();
        assert_eq!(
            last(&store, BackupTrigger::Scheduled),
            Some(scheduled.created_at)
        );
    }
}
