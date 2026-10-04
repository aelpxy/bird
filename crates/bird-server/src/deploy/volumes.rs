use bird_api::{DeployRequest, VolumeSpec};
use bird_core::{EnvironmentId, ImageLineage, ImageRef, Volume};
use bird_podman::VolumeMount;

use crate::state::AppState;
use crate::{Error, Result, labels};

// checks that run before any config is saved, so a refused deploy changes nothing
pub(super) async fn preflight(
    state: &AppState,
    environment: EnvironmentId,
    request: &DeployRequest,
) -> Result<Vec<VolumeSpec>> {
    let name = request.name.clone();
    let (service, existing) = state
        .db
        .call(move |store| {
            let Some(service) = store.service_by_name(environment, &name)? else {
                return Ok((None, Vec::new()));
            };
            let volumes = store.list_volumes(service.id)?;
            Ok((Some(service), volumes))
        })
        .await?;

    let new_volumes = plan(&existing, &request.volumes)?;
    let has_volumes = !existing.is_empty() || !new_volumes.is_empty();
    let replicas = request.replicas.or(service.map(|s| s.replicas));
    if has_volumes && replicas.is_some_and(|r| r.get() > 1) {
        return Err(Error::VolumeNeedsSingleMachine(request.name.clone()));
    }
    check_lineage(&existing, &request.image, request.allow_image_change)?;
    Ok(new_volumes)
}

pub(super) async fn mounts(state: &AppState, volumes: &[Volume]) -> Result<Vec<VolumeMount>> {
    let mut mounts = Vec::with_capacity(volumes.len());
    for volume in volumes {
        let podman_name = volume.podman_name();
        if volume.lineage.is_none() {
            state
                .podman
                .ensure_volume(&podman_name, &labels::for_volume(volume))
                .await?;
        } else if !state.podman.volume_exists(&podman_name).await? {
            // podman would silently create an empty one, which looks exactly like lost data
            return Err(Error::VolumeMissing(volume.name.clone()));
        }
        mounts.push(VolumeMount {
            volume: podman_name,
            destination: volume.mount_path.to_string(),
        });
    }
    Ok(mounts)
}

pub(super) async fn record_lineage(state: &AppState, volumes: &[Volume], image: &ImageRef) {
    let lineage = ImageLineage::of(image).to_string();
    for volume in volumes {
        if volume.lineage.as_deref() == Some(lineage.as_str()) {
            continue;
        }
        let (id, recorded) = (volume.id, lineage.clone());
        let saved = state
            .db
            .call(move |store| store.set_volume_lineage(id, &recorded))
            .await;
        if let Err(err) = saved {
            tracing::warn!(volume = %volume.name, error = %err, "could not record volume image");
        }
    }
}

fn plan(existing: &[Volume], requested: &[VolumeSpec]) -> Result<Vec<VolumeSpec>> {
    let mut new_volumes = Vec::new();
    for spec in requested {
        match existing.iter().find(|v| v.name == spec.name) {
            Some(volume) if volume.mount_path != spec.path => {
                return Err(Error::VolumePathChanged {
                    volume: spec.name.clone(),
                    was: volume.mount_path.clone(),
                    now: spec.path.clone(),
                });
            }
            Some(_) => {}
            None => new_volumes.push(spec.clone()),
        }
    }
    Ok(new_volumes)
}

fn check_lineage(existing: &[Volume], image: &ImageRef, allow_change: bool) -> Result<()> {
    if allow_change {
        return Ok(());
    }
    let now = ImageLineage::of(image);
    match existing
        .iter()
        .find(|v| v.lineage.as_ref().is_some_and(|was| !now.matches(was)))
    {
        Some(volume) => Err(Error::ImageChange {
            volume: volume.name.clone(),
            was: volume.lineage.clone().unwrap_or_default(),
            now: now.to_string(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{ServiceId, VolumeId};

    use super::*;

    fn volume(name: &str, path: &str, lineage: Option<&str>) -> Volume {
        Volume {
            id: VolumeId::generate(),
            service_id: ServiceId::generate(),
            name: name.parse().unwrap(),
            mount_path: path.parse().unwrap(),
            lineage: lineage.map(str::to_owned),
            created_at: 0,
        }
    }

    fn spec(name: &str, path: &str) -> VolumeSpec {
        VolumeSpec {
            name: name.parse().unwrap(),
            path: path.parse().unwrap(),
        }
    }

    #[test]
    fn plans_only_new_volumes_and_refuses_moving_one() {
        let existing = [volume("data", "/var/lib/postgresql", None)];
        let new_volumes = plan(
            &existing,
            &[spec("data", "/var/lib/postgresql"), spec("logs", "/logs")],
        )
        .unwrap();
        assert_eq!(new_volumes, vec![spec("logs", "/logs")]);
        assert!(matches!(
            plan(&existing, &[spec("data", "/elsewhere")]),
            Err(Error::VolumePathChanged { .. })
        ));
    }

    #[test]
    fn guards_against_image_lineage_changes() {
        let image = |raw: &str| raw.parse::<ImageRef>().unwrap();
        let existing = [volume("data", "/d", Some("docker.io/library/postgres:18"))];
        assert!(check_lineage(&existing, &image("postgres:18.7"), false).is_ok());
        assert!(matches!(
            check_lineage(&existing, &image("postgres:19"), false),
            Err(Error::ImageChange { .. })
        ));
        assert!(matches!(
            check_lineage(&existing, &image("postgres:18-alpine"), false),
            Err(Error::ImageChange { .. })
        ));
        assert!(check_lineage(&existing, &image("postgres:19"), true).is_ok());
        let unused = [volume("data", "/d", None)];
        assert!(check_lineage(&unused, &image("anything:1"), false).is_ok());
        let built = vec![volume(
            "data",
            "/data",
            Some("localhost/bird/notes:1791077958876"),
        )];
        assert!(check_lineage(&built, &image("localhost/bird/notes:1791082579769"), false).is_ok());
    }
}
