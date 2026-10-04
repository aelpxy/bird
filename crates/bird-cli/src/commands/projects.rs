use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::{CreateEnvironment, CreateProject, ProjectSummary};
use bird_core::Name;

use super::table::render;
use crate::args::{EnvironmentCommand, ProjectCommand};
use crate::client::ApiClient;
use crate::profile;
use crate::scope::{FIRST_ENVIRONMENT, Scope};
use crate::ui::style::{self, Paint};
use crate::ui::{Output, prompt};

const TIMEOUT: Duration = Duration::from_secs(30);
const CURRENT: &str = "*";

pub(crate) async fn project(
    client: &ApiClient,
    command: ProjectCommand,
    out: Output,
) -> Result<()> {
    match command {
        ProjectCommand::List => {
            let projects = list(client).await?;
            if out.json(&projects)? {
                return Ok(());
            }
            let current = client.scope();
            let rows: Vec<Vec<String>> = projects
                .into_iter()
                .map(|project| {
                    let here = project.name == current.project;
                    let environments: Vec<String> = project
                        .environments
                        .iter()
                        .map(|env| marked(env, here && *env == current.environment))
                        .collect();
                    vec![marked(&project.name, here), environments.join(", ")]
                })
                .collect();
            print!("{}", render(&["PROJECT", "ENVIRONMENTS"], &rows));
        }
        ProjectCommand::Create { name } => {
            let created: ProjectSummary = client
                .post(
                    "/v1/projects",
                    &CreateProject { name: name.clone() },
                    TIMEOUT,
                )
                .await?;
            if out.json(&created)? {
                return Ok(());
            }
            println!(
                "{} created project {name} with environment {FIRST_ENVIRONMENT}; act in it with `bird switch {name}`",
                style::out(Paint::Green, "✓")
            );
        }
        ProjectCommand::Remove { name, yes } => {
            prompt::confirm(&format!("delete project {name}?"), yes)?;
            client
                .delete(&format!("/v1/projects/{name}"), TIMEOUT)
                .await?;
            println!("{} deleted project {name}", style::out(Paint::Green, "✓"));
        }
    }
    Ok(())
}

pub(crate) async fn environment(
    client: &ApiClient,
    command: EnvironmentCommand,
    out: Output,
) -> Result<()> {
    let current = client.scope();
    let project = &current.project;
    match command {
        EnvironmentCommand::List => {
            let projects = list(client).await?;
            let found = find(&projects, project)?;
            if out.json(&found.environments)? {
                return Ok(());
            }
            let rows: Vec<Vec<String>> = found
                .environments
                .iter()
                .map(|env| vec![marked(env, *env == current.environment)])
                .collect();
            print!("{}", render(&["ENVIRONMENT"], &rows));
        }
        EnvironmentCommand::Create { name } => {
            let created: Name = client
                .post(
                    &format!("/v1/projects/{project}/environments"),
                    &CreateEnvironment { name: name.clone() },
                    TIMEOUT,
                )
                .await?;
            if out.json(&created)? {
                return Ok(());
            }
            println!(
                "{} created {project}/{name}; act in it with -E {name} or `bird switch {project} {name}`",
                style::out(Paint::Green, "✓")
            );
        }
        EnvironmentCommand::Remove { name, yes } => {
            prompt::confirm(&format!("delete environment {project}/{name}?"), yes)?;
            client
                .delete(
                    &format!("/v1/projects/{project}/environments/{name}"),
                    TIMEOUT,
                )
                .await?;
            println!("{} deleted {project}/{name}", style::out(Paint::Green, "✓"));
        }
    }
    Ok(())
}

// saved with the login, so it follows that server
pub(crate) async fn switch(
    client: &ApiClient,
    project: Name,
    environment: Option<Name>,
) -> Result<()> {
    let environment = match environment {
        Some(environment) => environment,
        None => FIRST_ENVIRONMENT.parse()?,
    };
    let projects = list(client).await?;
    let found = find(&projects, &project)?;
    if !found.environments.contains(&environment) {
        bail!(
            "project {project} has no environment {environment}, see `bird environment list -p {project}`"
        );
    }
    let path = profile::path().context("cannot find a config directory, set HOME")?;
    let Some(mut saved) = profile::load(&path)? else {
        bail!(
            "log in first, the choice is saved with the login: bird login <host:port> < api-token"
        );
    };
    saved.project = Some(project.clone());
    saved.environment = Some(environment.clone());
    profile::save(&path, &saved)?;
    let scope = Scope {
        project,
        environment,
    };
    println!("{} now acting in {scope}", style::out(Paint::Green, "✓"));
    Ok(())
}

async fn list(client: &ApiClient) -> Result<Vec<ProjectSummary>> {
    client.get("/v1/projects", TIMEOUT).await
}

fn find<'a>(projects: &'a [ProjectSummary], name: &Name) -> Result<&'a ProjectSummary> {
    projects
        .iter()
        .find(|project| project.name == *name)
        .with_context(|| format!("project {name} not found, see `bird project list`"))
}

fn marked(name: &Name, current: bool) -> String {
    if current {
        format!("{name}{CURRENT}")
    } else {
        name.to_string()
    }
}
