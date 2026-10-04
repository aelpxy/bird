use std::time::Duration;

use anyhow::Result;
use bird_api::{CreateFromTemplate, TemplateDeployResponse, TemplateSummary};
use bird_core::Name;

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use super::table::render;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, Spinner};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn list(client: &ApiClient, out: Output) -> Result<()> {
    let templates: Vec<TemplateSummary> = client.get("/v1/templates", TIMEOUT).await?;
    if out.json(&templates)? {
        return Ok(());
    }
    let rows: Vec<Vec<String>> = templates
        .into_iter()
        .map(|t| {
            vec![
                style::out(Paint::Bold, t.name),
                t.image.to_string(),
                t.description,
            ]
        })
        .collect();
    print!("{}", render(&["TEMPLATE", "IMAGE", "DESCRIPTION"], &rows));
    Ok(())
}

pub(crate) async fn add(
    client: &ApiClient,
    template: &Name,
    name: Option<Name>,
    out: Output,
) -> Result<()> {
    let service = name.clone().unwrap_or_else(|| template.clone());
    let spinner = Spinner::start(format!("creating {service} from the {template} template"));
    let response: TemplateDeployResponse = client
        .post(
            &client.scoped(&format!("templates/{template}/deploy")),
            &CreateFromTemplate { name },
            DEPLOY_TIMEOUT,
        )
        .await?;
    let elapsed = spinner.elapsed();
    drop(spinner);
    if out.json(&response)? {
        return Ok(());
    }
    print_deployed(&response.deployment, elapsed, Output { json: false })?;
    if let Some(connection) = response.connection {
        println!("\nconnect a service to it:");
        println!(
            "  {}",
            style::out(
                Paint::Bold,
                format!("bird env set -s <service> '{connection}=${{{{{service}.{connection}}}}}'")
            )
        );
    }
    Ok(())
}
