use std::time::Duration;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use bird_api::{DeployResponse, MANIFEST_FILE, Manifest};
use bird_core::{BuildFile, Command, EnvKey, ImageRef};

use super::build;
use crate::args::DeployArgs;
use crate::client::ApiClient;
use crate::manifest::{self, Loaded};

// covers a slow image pull plus the app's startup window on the server
pub(crate) const DEPLOY_TIMEOUT: Duration = Duration::from_mins(15);

pub(crate) async fn run(client: &ApiClient, args: DeployArgs) -> Result<()> {
    let loaded = manifest::load(args.config.as_deref())?;
    let allow_image_change = args.allow_image_change;
    let (manifest, source) = merge(loaded, args)?;
    let image = match source {
        Source::Image(image) => image,
        Source::Build {
            dir,
            dockerfile,
            args,
        } => build::run(client, &manifest.name, &dir, dockerfile, &args).await?,
    };
    let mut request = manifest.into_request(image);
    request.allow_image_change = allow_image_change;
    println!("deploying {} ({})...", request.name, request.image);
    let response: DeployResponse = client.post("/v1/deploy", &request, DEPLOY_TIMEOUT).await?;
    print_deployed(&response);
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Source {
    Image(ImageRef),
    Build {
        dir: PathBuf,
        dockerfile: Option<BuildFile>,
        args: BTreeMap<EnvKey, String>,
    },
}

// flags win over bird.toml; repeatable flags add to its lists
fn merge(loaded: Option<Loaded>, args: DeployArgs) -> Result<(Manifest, Source)> {
    let (mut manifest, dir) = match (loaded, args.name.clone()) {
        (Some(Loaded { mut manifest, dir }), name) => {
            manifest.name = name.unwrap_or(manifest.name);
            (manifest, dir)
        }
        (None, Some(name)) => (Manifest::named(name), PathBuf::new()),
        (None, None) => bail!(
            "no {MANIFEST_FILE} here: pass a name and image, or create one with `bird init <name> <image>`"
        ),
    };
    let dockerfile = manifest.build.as_ref().and_then(|b| b.dockerfile.clone());
    let mut build_args = manifest
        .build
        .as_ref()
        .map(|b| b.args.clone())
        .unwrap_or_default();
    build_args.extend(args.build_args);
    let source = match (
        args.image,
        args.build,
        manifest.image.take(),
        manifest.build.take(),
    ) {
        (Some(image), ..) | (None, None, Some(image), _) => Source::Image(image),
        (None, Some(build_dir), ..) => Source::Build {
            dir: build_dir,
            dockerfile,
            args: build_args,
        },
        (None, None, None, Some(build)) => Source::Build {
            dir: context_dir(&dir, build.context),
            dockerfile,
            args: build_args,
        },
        (None, None, None, None) => {
            bail!(
                "nothing to deploy: pass an image or --build, or set image or [build] in {MANIFEST_FILE}"
            )
        }
    };
    if let Some(port) = args.port {
        manifest.port = Some(port);
    }
    for domain in args.domains {
        if !manifest.domains.contains(&domain) {
            manifest.domains.push(domain);
        }
    }
    manifest.env.extend(args.env);
    for volume in args.volumes {
        manifest.volumes.retain(|v| v.name != volume.name);
        manifest.volumes.push(volume);
    }
    if !args.command.is_empty() {
        manifest.command = Some(Command::try_from(args.command)?);
    }
    manifest.health = args.health.or(manifest.health);
    manifest.health_timeout = args.health_timeout.or(manifest.health_timeout);
    manifest.memory = args.memory.or(manifest.memory);
    manifest.cpus = args.cpus.or(manifest.cpus);
    manifest.replicas = args.replicas.or(manifest.replicas);
    Ok((manifest, source))
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

fn context_dir(manifest_dir: &Path, context: Option<PathBuf>) -> PathBuf {
    let dir = manifest_dir.join(context.unwrap_or_default());
    if dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        dir
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
        *deploy
    }

    fn loaded(source: &str) -> Loaded {
        Loaded {
            manifest: toml::from_str(source).unwrap(),
            dir: PathBuf::from("app"),
        }
    }

    const WITH_IMAGE: &str = r#"
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
"#;

    const WITH_BUILD: &str = r#"
name = "web"
[build]
context = "server"
dockerfile = "docker/Dockerfile"
[build.args]
NODE_ENV = "production"
MODE = "file"
"#;

    #[test]
    fn flags_override_the_manifest() {
        let (manifest, source) = merge(
            Some(loaded(WITH_IMAGE)),
            args(&[
                "--port",
                "8080",
                "--domain",
                "api.localhost",
                "-e",
                "B=flag",
                "-v",
                "data:/srv",
                "--replicas",
                "2",
                "--",
                "serve",
            ]),
        )
        .unwrap();
        assert_eq!(source, Source::Image("app:1".parse().unwrap()));
        let request = manifest.into_request("app:1".parse().unwrap());
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
        assert_eq!(request.replicas.map(bird_core::Replicas::get), Some(2));
    }

    #[test]
    fn positional_name_and_image_win() {
        let (manifest, source) =
            merge(Some(loaded(WITH_BUILD)), args(&["staging", "app:2"])).unwrap();
        assert_eq!(manifest.name.as_str(), "staging");
        assert_eq!(source, Source::Image("app:2".parse().unwrap()));
    }

    #[test]
    fn builds_relative_to_the_manifest() {
        let (_, source) = merge(
            Some(loaded(WITH_BUILD)),
            args(&["--build-arg", "MODE=flag"]),
        )
        .unwrap();
        let expected_args = BTreeMap::from([
            ("MODE".parse().unwrap(), "flag".to_owned()),
            ("NODE_ENV".parse().unwrap(), "production".to_owned()),
        ]);
        assert_eq!(
            source,
            Source::Build {
                dir: PathBuf::from("app/server"),
                dockerfile: Some("docker/Dockerfile".parse().unwrap()),
                args: expected_args,
            }
        );
        let (_, source) = merge(Some(loaded(WITH_IMAGE)), args(&["--build"])).unwrap();
        assert_eq!(
            source,
            Source::Build {
                dir: PathBuf::from("."),
                dockerfile: None,
                args: BTreeMap::new(),
            }
        );
        assert!(Args::try_parse_from(["bird", "deploy", "web", "app:1", "--build"]).is_err());
        assert_eq!(context_dir(Path::new(""), None), PathBuf::from("."));
    }

    #[test]
    fn needs_a_manifest_or_name_and_source() {
        assert!(merge(None, args(&[])).is_err());
        assert!(merge(None, args(&["web"])).is_err());
        let (manifest, source) = merge(None, args(&["web", "nginx"])).unwrap();
        assert_eq!(source, Source::Image("nginx".parse().unwrap()));
        assert_eq!(
            manifest.into_request("nginx".parse().unwrap()).port.get(),
            80
        );
        let (_, source) = merge(None, args(&["web", "--build", "site"])).unwrap();
        assert!(matches!(source, Source::Build { dir, .. } if dir == Path::new("site")));
    }
}
