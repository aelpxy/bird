use std::time::Duration;

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use bird_api::{DeployRequest, DeployResponse, MANIFEST_FILE, Manifest};
use bird_core::Command;

use crate::args::DeployArgs;
use crate::client::ApiClient;
use crate::manifest;

// covers a slow image pull plus the app's startup window on the server
pub(crate) const DEPLOY_TIMEOUT: Duration = Duration::from_mins(15);

pub(crate) async fn run(client: &ApiClient, args: DeployArgs) -> Result<()> {
    let manifest = manifest::load(args.config.as_deref())?;
    let request = merge(manifest, args)?;
    println!("deploying {} ({})...", request.name, request.image);
    let response: DeployResponse = client.post("/v1/deploy", &request, DEPLOY_TIMEOUT).await?;
    print_deployed(&response);
    Ok(())
}

// flags win over bird.toml; repeatable flags add to its lists
fn merge(manifest: Option<Manifest>, args: DeployArgs) -> Result<DeployRequest> {
    let mut request = match (manifest, args.name, args.image) {
        (Some(manifest), name, image) => {
            let mut request = manifest.into_request();
            request.name = name.unwrap_or(request.name);
            request.image = image.unwrap_or(request.image);
            request
        }
        (None, Some(name), Some(image)) => Manifest {
            name,
            image,
            port: None,
            domains: Vec::new(),
            health: None,
            command: None,
            memory: None,
            cpus: None,
            env: BTreeMap::new(),
            volumes: Vec::new(),
        }
        .into_request(),
        (None, _, _) => bail!(
            "no {MANIFEST_FILE} here: pass a name and image, or create one with `bird init <name> <image>`"
        ),
    };
    if let Some(port) = args.port {
        request.port = port;
    }
    for domain in args.domains {
        if !request.domains.contains(&domain) {
            request.domains.push(domain);
        }
    }
    request.env.extend(args.env);
    for volume in args.volumes {
        request.volumes.retain(|v| v.name != volume.name);
        request.volumes.push(volume);
    }
    if !args.command.is_empty() {
        request.command = Some(Command::try_from(args.command)?);
    }
    request.health = args.health.or(request.health);
    request.memory = args.memory.or(request.memory);
    request.cpus = args.cpus.or(request.cpus);
    request.allow_image_change = args.allow_image_change;
    Ok(request)
}

pub(crate) fn print_deployed(response: &DeployResponse) {
    println!(
        "deployed {} (deployment {})",
        response.service, response.deployment_id
    );
    for domain in &response.domains {
        println!("  -> {domain}");
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::args::{Args, Command as CliCommand};

    fn args(argv: &[&str]) -> DeployArgs {
        let argv = ["bird", "deploy"].iter().chain(argv);
        let CliCommand::Deploy(deploy) = Args::try_parse_from(argv).unwrap().command else {
            panic!("expected deploy");
        };
        deploy
    }

    fn manifest() -> Manifest {
        toml::from_str(
            r#"
name = "web"
image = "app:1"
port = 3000
domains = ["web.localhost"]
memory = "512m"
[env]
A = "file"
B = "file"
[[volumes]]
name = "data"
path = "/data"
"#,
        )
        .unwrap()
    }

    #[test]
    fn flags_override_the_manifest() {
        let request = merge(
            Some(manifest()),
            args(&[
                "--port",
                "8080",
                "--domain",
                "api.localhost",
                "-e",
                "B=flag",
                "-v",
                "data:/srv",
                "--",
                "serve",
            ]),
        )
        .unwrap();
        assert_eq!(request.name.as_str(), "web");
        assert_eq!(request.port.get(), 8080);
        assert_eq!(request.domains.len(), 2);
        assert_eq!(request.env[&"A".parse().unwrap()], "file");
        assert_eq!(request.env[&"B".parse().unwrap()], "flag");
        assert_eq!(request.volumes.len(), 1);
        assert_eq!(request.volumes[0].path.to_string(), "/srv");
        assert_eq!(
            request.memory.map(bird_core::MemoryLimit::mebibytes),
            Some(512)
        );
        assert!(request.command.is_some());
    }

    #[test]
    fn positional_name_and_image_win() {
        let request = merge(Some(manifest()), args(&["staging", "app:2"])).unwrap();
        assert_eq!(request.name.as_str(), "staging");
        assert_eq!(request.image.as_str(), "app:2");
    }

    #[test]
    fn needs_a_manifest_or_name_and_image() {
        assert!(merge(None, args(&[])).is_err());
        assert!(merge(None, args(&["web"])).is_err());
        let request = merge(None, args(&["web", "nginx"])).unwrap();
        assert_eq!(request.port.get(), 80);
    }
}
