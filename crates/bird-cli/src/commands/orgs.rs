use std::time::Duration;

use anyhow::Result;
use bird_api::{CreateOrg, OrgMember, OrgSummary, SetMember};

use super::table::render;
use crate::args::OrgCommand;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, prompt};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(client: &ApiClient, command: OrgCommand, out: Output) -> Result<()> {
    match command {
        OrgCommand::List => {
            let orgs: Vec<OrgSummary> = client.get("/v1/orgs", TIMEOUT).await?;
            if out.json(&orgs)? {
                return Ok(());
            }
            if orgs.is_empty() {
                println!("you belong to no org yet, ask an org owner to add you");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = orgs
                .into_iter()
                .map(|org| {
                    let role = org
                        .role
                        .map_or_else(|| style::out(Paint::Dim, "-"), |role| role.to_string());
                    vec![org.name.to_string(), role]
                })
                .collect();
            print!("{}", render(&["ORG", "ROLE"], &rows));
        }
        OrgCommand::Create { name } => {
            let created: OrgSummary = client
                .post("/v1/orgs", &CreateOrg { name }, TIMEOUT)
                .await?;
            if out.json(&created)? {
                return Ok(());
            }
            println!(
                "{} created org {}; add people with `bird org add {} <user>`",
                style::out(Paint::Green, "✓"),
                created.name,
                created.name
            );
        }
        OrgCommand::Delete { name, yes } => {
            prompt::confirm(&format!("delete org {name}?"), yes)?;
            client.delete(&format!("/v1/orgs/{name}"), TIMEOUT).await?;
            println!("{} deleted org {name}", style::out(Paint::Green, "✓"));
        }
        OrgCommand::Members { org } => {
            let members: Vec<OrgMember> = client
                .get(&format!("/v1/orgs/{org}/members"), TIMEOUT)
                .await?;
            if out.json(&members)? {
                return Ok(());
            }
            if members.is_empty() {
                println!("{org} has no members yet, add one with `bird org add {org} <user>`");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = members
                .into_iter()
                .map(|member| vec![member.user.to_string(), member.role.to_string()])
                .collect();
            print!("{}", render(&["USER", "ROLE"], &rows));
        }
        OrgCommand::Add { org, user, role } => {
            client
                .put(
                    &format!("/v1/orgs/{org}/members/{user}"),
                    &SetMember { role },
                    TIMEOUT,
                )
                .await?;
            println!(
                "{} {user} is now {role} of {org}",
                style::out(Paint::Green, "✓")
            );
        }
        OrgCommand::Remove { org, user, yes } => {
            prompt::confirm(&format!("remove {user} from {org}?"), yes)?;
            client
                .delete(&format!("/v1/orgs/{org}/members/{user}"), TIMEOUT)
                .await?;
            println!(
                "{} removed {user} from {org}",
                style::out(Paint::Green, "✓")
            );
        }
    }
    Ok(())
}
