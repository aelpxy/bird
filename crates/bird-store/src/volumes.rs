use bird_core::{MountPath, Name, ServiceId, Volume, VolumeId};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn create_volume(
        &self,
        service_id: ServiceId,
        name: &Name,
        mount_path: &MountPath,
    ) -> Result<Volume> {
        let volume = Volume {
            id: VolumeId::generate(),
            service_id,
            name: name.clone(),
            mount_path: mount_path.clone(),
            lineage: None,
            created_at: now(),
        };
        self.execute(
            "INSERT INTO volumes (id, service_id, name, mount_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                volume.id.to_string(),
                service_id.to_string(),
                volume.name.as_str(),
                volume.mount_path.as_str(),
                volume.created_at
            ],
        )
        .map_err(write_error("volume"))?;
        Ok(volume)
    }

    pub fn list_volumes(&self, service_id: ServiceId) -> Result<Vec<Volume>> {
        self.query_all(
            "SELECT id, service_id, name, mount_path, lineage, created_at FROM volumes
             WHERE service_id = ?1 ORDER BY name",
            [service_id.to_string()],
            rows::volume,
        )
    }

    pub fn set_volume_lineage(&self, id: VolumeId, lineage: &str) -> Result<()> {
        let changed = self.execute(
            "UPDATE volumes SET lineage = ?2 WHERE id = ?1",
            params![id.to_string(), lineage],
        )?;
        expect_changed(changed, "volume")
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{MountPath, Name};

    use crate::Error;
    use crate::testing::setup;

    #[test]
    fn creates_lists_and_records_lineage() {
        let (store, service) = setup();
        let name: Name = "data".parse().unwrap();
        let path: MountPath = "/var/lib/postgresql".parse().unwrap();
        let volume = store.create_volume(service.id, &name, &path).unwrap();
        assert_eq!(volume.lineage, None);
        assert!(volume.podman_name().starts_with("bird-volume-"));

        store
            .set_volume_lineage(volume.id, "docker.io/library/postgres:18")
            .unwrap();
        let listed = store.list_volumes(service.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].lineage.as_deref(),
            Some("docker.io/library/postgres:18")
        );
    }

    #[test]
    fn names_and_paths_are_unique_per_service() {
        let (store, service) = setup();
        let path: MountPath = "/data".parse().unwrap();
        store
            .create_volume(service.id, &"a".parse().unwrap(), &path)
            .unwrap();
        assert!(matches!(
            store
                .create_volume(
                    service.id,
                    &"a".parse().unwrap(),
                    &"/other".parse().unwrap()
                )
                .unwrap_err(),
            Error::AlreadyExists("volume")
        ));
        assert!(matches!(
            store
                .create_volume(service.id, &"b".parse().unwrap(), &path)
                .unwrap_err(),
            Error::AlreadyExists("volume")
        ));
    }
}
