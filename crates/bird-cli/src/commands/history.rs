use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bird_api::{DeployResponse, DeploymentInfo, RollbackRequest};
use bird_core::{DeploymentId, Name};

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use super::table::render;
use crate::client::ApiClient;
use crate::ui::style;
use crate::ui::{Output, Spinner};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn history(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let deployments: Vec<DeploymentInfo> = client
        .get(&format!("/v1/services/{name}/deployments"), TIMEOUT)
        .await?;
    if out.json(&deployments)? {
        return Ok(());
    }
    if deployments.is_empty() {
        println!("{name} has no deployments yet");
        return Ok(());
    }
    let now = unix_now();
    let rows: Vec<Vec<String>> = deployments
        .iter()
        .map(|d| {
            vec![
                d.id.to_string(),
                style::out(style::deployment(d.status), d.status),
                d.image.to_string(),
                d.variables.to_string(),
                ago(now.saturating_sub(d.created_at)),
            ]
        })
        .collect();
    print!(
        "{}",
        render(&["DEPLOYMENT", "STATUS", "IMAGE", "VARS", "CREATED"], &rows)
    );
    Ok(())
}

pub(crate) async fn rollback(
    client: &ApiClient,
    name: &Name,
    deployment_id: Option<DeploymentId>,
    out: Output,
) -> Result<()> {
    let target = match deployment_id {
        Some(id) => format!("deployment {id}"),
        None => "its previous deployment".to_owned(),
    };
    let spinner = Spinner::start(format!(
        "rolling {name} back to {target} (image, port and variables)"
    ));
    let request = RollbackRequest { deployment_id };
    let response: DeployResponse = client
        .post(
            &format!("/v1/services/{name}/rollback"),
            &request,
            DEPLOY_TIMEOUT,
        )
        .await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    print_deployed(&response, elapsed, out)
}

pub(super) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub(super) fn ago(seconds: i64) -> String {
    match seconds {
        ..60 => format!("{}s ago", seconds.max(0)),
        60..3600 => format!("{}m ago", seconds / 60),
        3600..86_400 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_ages() {
        assert_eq!(ago(-5), "0s ago");
        assert_eq!(ago(59), "59s ago");
        assert_eq!(ago(61), "1m ago");
        assert_eq!(ago(7200), "2h ago");
        assert_eq!(ago(200_000), "2d ago");
    }
}
