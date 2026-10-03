use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bird_api::BuildEvent;
use bird_core::{BuildFile, ImageRef, Name};

use crate::client::ApiClient;
use crate::context;

// covers uploading the context; birdd bounds the build itself
const UPLOAD_TIMEOUT: Duration = Duration::from_mins(10);
const DEFAULT_DOCKERFILE: &str = "Dockerfile";

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    dir: &Path,
    dockerfile: Option<BuildFile>,
) -> Result<ImageRef> {
    let dockerfile = match dockerfile {
        Some(dockerfile) => dockerfile,
        None => DEFAULT_DOCKERFILE.parse()?,
    };
    let packed = context::pack(dir, &dockerfile)?;
    println!(
        "uploading {} ({} files, {:.1} MiB)...",
        dir.display(),
        packed.files,
        mebibytes(packed.archive.len())
    );
    let path = format!("/v1/services/{name}/builds?dockerfile={dockerfile}");
    let mut outcome = None;
    client
        .upload_lines(
            &path,
            "application/x-tar",
            packed.archive,
            UPLOAD_TIMEOUT,
            |line| {
                let event: BuildEvent =
                    serde_json::from_str(line).context("birdd sent an unexpected build event")?;
                match event {
                    BuildEvent::Log { line } => println!("  {line}"),
                    BuildEvent::Built { image } => outcome = Some(Ok(image)),
                    BuildEvent::Failed { error } => outcome = Some(Err(anyhow!(error))),
                }
                Ok(())
            },
        )
        .await?;
    match outcome {
        Some(Ok(image)) => {
            println!("built {image}");
            Ok(image)
        }
        Some(Err(err)) => Err(err),
        None => bail!("the build stream ended without a result"),
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "an approximate size is all that is shown"
)]
fn mebibytes(bytes: usize) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}
