use anyhow::Result;
use bird_api::ServiceSummary;
use bird_core::Name;

use super::deploy::DEPLOY_TIMEOUT;
use super::scale::{Converged, converge};
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, Spinner, duration};

pub(crate) async fn stop(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let spinner = Spinner::start(format!("stopping {name}"));
    let summary: ServiceSummary = client
        .post(&format!("/v1/services/{name}/stop"), &(), DEPLOY_TIMEOUT)
        .await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    if out.json(&summary)? {
        return Ok(());
    }
    println!(
        "{} stopped {name} in {}",
        style::out(Paint::Green, "✓"),
        duration(elapsed)
    );
    println!(
        "  {}",
        style::out(
            Paint::Dim,
            "its machines, data and settings are kept; `bird start` brings it back"
        )
    );
    Ok(())
}

pub(crate) async fn start(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let spinner = Spinner::start(format!("starting {name}"));
    let summary: ServiceSummary = client
        .post(&format!("/v1/services/{name}/start"), &(), DEPLOY_TIMEOUT)
        .await?;
    // replicas without a stopped machine to start come from the supervisor shortly after
    let reached = converge(client, name, &spinner).await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    if out.json(&summary)? {
        return Ok(());
    }
    match reached {
        Converged::Running(running) => println!(
            "{} started {name} in {}, {running}/{} running",
            style::out(Paint::Green, "✓"),
            duration(elapsed),
            summary.replicas
        ),
        Converged::NotDeployed | Converged::Stopped => {
            println!("{name} has nothing to run yet, deploy it first");
        }
    }
    Ok(())
}

pub(crate) async fn restart(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let spinner = Spinner::start(format!("restarting {name}, one machine at a time"));
    let summary: ServiceSummary = client
        .post(&format!("/v1/services/{name}/restart"), &(), DEPLOY_TIMEOUT)
        .await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    if out.json(&summary)? {
        return Ok(());
    }
    println!(
        "{} restarted {name} in {}",
        style::out(Paint::Green, "✓"),
        duration(elapsed)
    );
    Ok(())
}
