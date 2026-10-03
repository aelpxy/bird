use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bird_api::{DeployResponse, DeploymentInfo, RollbackRequest};
use bird_core::{DeploymentId, Name};

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use super::table::render;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn history(client: &ApiClient, name: &Name) -> Result<()> {
    let deployments: Vec<DeploymentInfo> = client
        .get(&format!("/v1/services/{name}/deployments"), TIMEOUT)
        .await?;
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
                d.status.to_string(),
                d.image.to_string(),
                ago(now.saturating_sub(d.created_at)),
            ]
        })
        .collect();
    print!(
        "{}",
        render(&["DEPLOYMENT", "STATUS", "IMAGE", "CREATED"], &rows)
    );
    Ok(())
}

pub(crate) async fn rollback(
    client: &ApiClient,
    name: &Name,
    deployment_id: Option<DeploymentId>,
) -> Result<()> {
    match deployment_id {
        Some(id) => println!("rolling {name} back to deployment {id}..."),
        None => println!("rolling {name} back to its previous deployment..."),
    }
    let request = RollbackRequest { deployment_id };
    let response: DeployResponse = client
        .post(
            &format!("/v1/services/{name}/rollback"),
            &request,
            DEPLOY_TIMEOUT,
        )
        .await?;
    print_deployed(&response);
    println!("note: rollbacks restore the image and port, variables stay as they are now");
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn ago(seconds: i64) -> String {
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
