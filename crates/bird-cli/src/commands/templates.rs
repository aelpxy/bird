use std::time::Duration;

use anyhow::Result;
use bird_api::{CreateFromTemplate, TemplateDeployResponse, TemplateSummary};
use bird_core::Name;

use super::deploy::{DEPLOY_TIMEOUT, print_deployed};
use super::table::render;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn list(client: &ApiClient) -> Result<()> {
    let templates: Vec<TemplateSummary> = client.get("/v1/templates", TIMEOUT).await?;
    let rows: Vec<Vec<String>> = templates
        .into_iter()
        .map(|t| vec![t.name.to_string(), t.image.to_string(), t.description])
        .collect();
    print!("{}", render(&["TEMPLATE", "IMAGE", "DESCRIPTION"], &rows));
    Ok(())
}

pub(crate) async fn add(client: &ApiClient, template: &Name, name: Option<Name>) -> Result<()> {
    let service = name.clone().unwrap_or_else(|| template.clone());
    println!("creating {service} from the {template} template...");
    let response: TemplateDeployResponse = client
        .post(
            &format!("/v1/templates/{template}/deploy"),
            &CreateFromTemplate { name },
            DEPLOY_TIMEOUT,
        )
        .await?;
    print_deployed(&response.deployment);
    if let Some(connection) = response.connection {
        println!("connect another service to it with:");
        println!("  bird env set <service> '{connection}=${{{{{service}.{connection}}}}}'");
    }
    Ok(())
}
