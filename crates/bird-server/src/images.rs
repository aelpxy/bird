use std::collections::BTreeSet;

use bird_core::ImageRef;

use crate::state::AppState;

// rollbacks to the last few deployments stay instant because their images are kept
const KEEP_RECENT_DEPLOYMENTS: u32 = 3;

pub(crate) async fn remove_unused(state: &AppState) {
    let images = match state
        .db
        .call(|store| store.list_deployed_images(KEEP_RECENT_DEPLOYMENTS))
        .await
    {
        Ok(images) => images,
        Err(err) => {
            tracing::warn!(error = %err, "could not list deployed images");
            return;
        }
    };
    let kept: BTreeSet<String> = images
        .iter()
        .filter(|(_, keep)| *keep)
        .map(|(image, _)| image.qualified())
        .collect();
    let unused = images
        .into_iter()
        .filter(|(_, keep)| !keep)
        .map(|(image, _)| image);
    remove(state, unused, &kept).await;
}

// images of a removed service go unless another service still deploys them
pub(crate) async fn remove_orphaned(state: &AppState, candidates: Vec<ImageRef>) {
    let still_deployed = match state
        .db
        .call(|store| store.list_deployed_images(u32::MAX))
        .await
    {
        Ok(images) => images.iter().map(|(image, _)| image.qualified()).collect(),
        Err(err) => {
            tracing::warn!(error = %err, "could not list deployed images");
            return;
        }
    };
    remove(state, candidates.into_iter(), &still_deployed).await;
}

async fn remove(
    state: &AppState,
    candidates: impl Iterator<Item = ImageRef>,
    kept: &BTreeSet<String>,
) {
    let mut seen = BTreeSet::new();
    for image in candidates {
        let qualified = image.qualified();
        if kept.contains(&qualified) || !seen.insert(qualified) {
            continue;
        }
        match state.podman.remove_image(&image).await {
            Ok(()) => tracing::info!(%image, "removed unused image"),
            Err(bird_podman::Error::NotFound { .. } | bird_podman::Error::Conflict { .. }) => {}
            Err(err) => tracing::warn!(%image, error = %err, "could not remove image"),
        }
    }
}
