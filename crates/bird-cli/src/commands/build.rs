use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bird_api::BuildEvent;
use bird_core::{BuildFile, EnvKey, ImageRef, Name};

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
    args: &BTreeMap<EnvKey, String>,
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
    let mut path = format!(
        "/v1/services/{name}/builds?dockerfile={}",
        encode(dockerfile.as_str())
    );
    if !args.is_empty() {
        path.push_str("&args=");
        path.push_str(&encode(&serde_json::to_string(args)?));
    }
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

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[allow(
    clippy::cast_precision_loss,
    reason = "an approximate size is all that is shown"
)]
fn mebibytes(bytes: usize) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("docker/web.Dockerfile"), "docker%2Fweb.Dockerfile");
        assert_eq!(encode("a&b=c d#"), "a%26b%3Dc%20d%23");
    }
}
