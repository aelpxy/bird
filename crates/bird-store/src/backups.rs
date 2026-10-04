use bird_core::{Backup, BackupId, EnvironmentId, Name};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Result, Store, rows};

const BACKUP_COLUMNS: &str =
    "SELECT id, environment_id, service_name, trigger, storage, created_at FROM backups";

impl Store {
    // stamps created_at, like every other record
    pub fn create_backup(&mut self, backup: Backup) -> Result<Backup> {
        let backup = Backup {
            created_at: now(),
            ..backup
        };
        self.transaction(|store| {
            store
                .execute(
                    "INSERT INTO backups (id, environment_id, service_name, trigger, storage, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        backup.id.to_string(),
                        backup.environment_id.to_string(),
                        backup.service.as_str(),
                        backup.trigger.as_str(),
                        backup.storage,
                        backup.created_at
                    ],
                )
                .map_err(write_error("backup"))?;
            for volume in &backup.volumes {
                store
                    .execute(
                        "INSERT INTO backup_volumes (backup_id, volume_name, lineage, key, size_bytes)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            backup.id.to_string(),
                            volume.name.as_str(),
                            volume.lineage,
                            volume.key,
                            i64::try_from(volume.size_bytes).unwrap_or(i64::MAX)
                        ],
                    )
                    .map_err(write_error("backup volume"))?;
            }
            Ok(())
        })?;
        Ok(backup)
    }

    pub fn backup(&self, id: BackupId) -> Result<Option<Backup>> {
        let found = self.query_one(
            &format!("{BACKUP_COLUMNS} WHERE id = ?1"),
            [id.to_string()],
            rows::backup,
        )?;
        found.map(|backup| self.with_volumes(backup)).transpose()
    }

    pub fn list_backups(
        &self,
        environment_id: EnvironmentId,
        service: &Name,
    ) -> Result<Vec<Backup>> {
        self.query_all(
            &format!(
                "{BACKUP_COLUMNS} WHERE environment_id = ?1 AND service_name = ?2 ORDER BY id DESC"
            ),
            params![environment_id.to_string(), service.as_str()],
            rows::backup,
        )?
        .into_iter()
        .map(|backup| self.with_volumes(backup))
        .collect()
    }

    pub fn delete_backup(&self, id: BackupId) -> Result<()> {
        let changed = self.execute("DELETE FROM backups WHERE id = ?1", [id.to_string()])?;
        expect_changed(changed, "backup")
    }

    fn with_volumes(&self, backup: Backup) -> Result<Backup> {
        let volumes = self.query_all(
            "SELECT volume_name, lineage, key, size_bytes FROM backup_volumes
             WHERE backup_id = ?1 ORDER BY volume_name",
            [backup.id.to_string()],
            rows::backup_volume,
        )?;
        Ok(Backup { volumes, ..backup })
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{Backup, BackupId, BackupTrigger, BackupVolume};

    use crate::Error;
    use crate::testing::{name, setup};

    fn backup(service: &bird_core::Service) -> Backup {
        let id = BackupId::generate();
        Backup {
            id,
            environment_id: service.environment_id,
            service: service.name.clone(),
            trigger: BackupTrigger::Manual,
            storage: "local".to_owned(),
            volumes: vec![BackupVolume {
                name: name("data"),
                lineage: Some("docker.io/library/postgres:18".to_owned()),
                key: format!("{id}/data.tar"),
                size_bytes: 4096,
            }],
            created_at: 0,
        }
    }

    #[test]
    fn stores_lists_newest_first_and_deletes() {
        let (mut store, service) = setup();
        let first = store.create_backup(backup(&service)).unwrap();
        let second = store.create_backup(backup(&service)).unwrap();
        assert!(first.created_at > 0);

        assert_eq!(store.backup(first.id).unwrap(), Some(first.clone()));
        let listed = store
            .list_backups(service.environment_id, &service.name)
            .unwrap();
        assert_eq!(listed, vec![second.clone(), first.clone()]);
        assert_eq!(
            store
                .list_backups(service.environment_id, &name("other"))
                .unwrap(),
            Vec::new()
        );

        store.delete_backup(first.id).unwrap();
        assert_eq!(store.backup(first.id).unwrap(), None);
        assert!(matches!(
            store.delete_backup(first.id).unwrap_err(),
            Error::NotFound("backup")
        ));
    }

    #[test]
    fn backups_outlive_their_service() {
        let (mut store, service) = setup();
        let kept = store.create_backup(backup(&service)).unwrap();
        store.delete_service(service.id).unwrap();
        assert_eq!(store.backup(kept.id).unwrap(), Some(kept));
    }
}
