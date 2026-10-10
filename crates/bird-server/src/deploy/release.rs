use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{Command, Deployment, EnvKey, Service};

use crate::state::AppState;
use crate::{Error, Result, commands};

// migrations can take a while, yet a deploy holds its service until this ends
const RELEASE_TIMEOUT: Duration = Duration::from_mins(10);
const FAILURE_LOG_LINES: usize = 30;

// runs once with the new deployment's image and variables before any of its machines start
pub(super) async fn run(
    state: &AppState,
    service: &Service,
    deployment: &Deployment,
    env: &BTreeMap<EnvKey, String>,
    release: &Command,
) -> Result<()> {
    let target =
        commands::RunTarget::for_deployment(state, service, &deployment.image, env).await?;
    tracing::info!(service = %service.name, deployment = %deployment.id, "running release command");
    let (result, output) = commands::run_collecting(state, &target, release, RELEASE_TIMEOUT).await;
    let reason = match result {
        Ok(Some(0)) => {
            tracing::info!(service = %service.name, deployment = %deployment.id, "release command succeeded");
            return Ok(());
        }
        Ok(Some(code)) => format!("exited with code {code}"),
        Ok(None) => "was interrupted".to_owned(),
        Err(Error::RunTimedOut(limit)) => format!("did not finish within {}s", limit.as_secs()),
        Err(err) => return Err(err),
    };
    Err(Error::ReleaseFailed {
        reason,
        logs: last_lines(&output),
    })
}

fn last_lines(output: &str) -> Vec<String> {
    let mut lines: Vec<String> = output
        .lines()
        .rev()
        .take(FAILURE_LOG_LINES)
        .map(str::to_owned)
        .collect();
    lines.reverse();
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_last_lines() {
        let output = (0..40)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = last_lines(&output);
        assert_eq!(lines.len(), FAILURE_LOG_LINES);
        assert_eq!(lines.first().unwrap(), "line 10");
        assert_eq!(lines.last().unwrap(), "line 39");
        assert_eq!(last_lines(""), Vec::<String>::new());
    }
}
