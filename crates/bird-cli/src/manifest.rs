use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bird_api::{MANIFEST_FILE, Manifest};
use bird_core::{EnvKey, ImageRef, Name, Port};

use crate::ui::style::{self, Paint};

const SECRET_HINTS: [&str; 5] = ["PASSWORD", "SECRET", "TOKEN", "API_KEY", "PRIVATE_KEY"];

pub(crate) struct Loaded {
    pub(crate) manifest: Manifest,
    // build contexts in bird.toml are relative to the file, not to where bird runs
    pub(crate) dir: PathBuf,
}

// an explicit --config must exist, the default bird.toml is optional
pub(crate) fn load(explicit: Option<&Path>) -> Result<Option<Loaded>> {
    let path = explicit.unwrap_or_else(|| Path::new(MANIFEST_FILE));
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) if explicit.is_none() && err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let manifest: Manifest =
        toml::from_str(&source).with_context(|| format!("invalid {}", path.display()))?;
    if manifest.image.is_some() && manifest.build.is_some() {
        bail!(
            "{} sets both image and [build], keep the one you deploy from",
            path.display()
        );
    }
    let dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    Ok(Some(Loaded { manifest, dir }))
}

// bird.toml is usually committed, so literal secrets in it end up in git history
pub(crate) fn warn_about_secrets(manifest: &Manifest, path: &Path) {
    let warning = style::err(Paint::Yellow, "warning:");
    let build_args = manifest.build.iter().flat_map(|build| &build.args);
    for (key, _) in build_args.filter(|(key, _)| looks_secret(key)) {
        eprintln!(
            "{warning} build arg {key} in {} looks like a secret, build args stay readable in the image",
            path.display()
        );
    }
    for (key, value) in &manifest.env {
        if looks_secret(key) && !value.is_empty() && !value.contains("${{") {
            eprintln!(
                "{warning} {key} in {} looks like a secret, set it with `bird env set -s {} {key}=...` instead",
                path.display(),
                manifest.name
            );
        }
    }
}

fn looks_secret(key: &EnvKey) -> bool {
    SECRET_HINTS.iter().any(|hint| key.as_str().contains(hint))
}

// where a new service's image comes from: a published image or the Dockerfile next to bird.toml
pub(crate) enum Starter<'a> {
    Image(&'a ImageRef),
    Build,
}

pub(crate) fn starter(name: &Name, source: &Starter<'_>, port: Port) -> String {
    let (image, build) = match source {
        Starter::Image(image) => (
            format!("image = \"{image}\"\n"),
            "# build from source on the server instead of pulling image; remove image above to use it
# [build]
# context = \".\"
# dockerfile = \"Dockerfile\"
# args = { NODE_ENV = \"production\" }",
        ),
        Starter::Build => (
            String::new(),
            "# built on the server from the Dockerfile; set image = \"...\" instead to pull one
[build]
context = \".\"
dockerfile = \"Dockerfile\"
# args = { NODE_ENV = \"production\" }",
        ),
    };
    format!(
        r#"name = "{name}"
{image}port = {port}
# domains = ["{name}.example.com"]
# health = "/healthz"
# health_timeout = "2m"
# memory = "512m"
# cpus = 0.5
# replicas = 2
# command = ["./server", "--listen", "0.0.0.0:{port}"]

{build}

# plain settings and references like ${{{{postgres.DATABASE_URL}}}}; set secrets with `bird env set`
[env]
# LOG_LEVEL = "info"

# [[volumes]]
# name = "data"
# path = "/data"

# back the volumes up on a schedule, keeping the newest copies
# [backup]
# every = "1d"
# keep = 7
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // the full example in the readme, so the docs fail the build when a field changes
    #[test]
    fn readme_example_parses() {
        let readme = include_str!("../../../README.md");
        let section = &readme[readme.find("### bird.toml").unwrap()..];
        let start = section.find("```toml\n").unwrap() + "```toml\n".len();
        let example = &section[start..start + section[start..].find("```").unwrap()];
        let manifest: Manifest = toml::from_str(example).unwrap();
        assert_eq!(manifest.name.as_str(), "api");
        assert!(manifest.image.is_some() && manifest.build.is_none());
        assert!(manifest.health_timeout.is_some() && manifest.backup.is_some());
        assert_eq!(manifest.volumes.len(), 1);
        assert_eq!(manifest.cron.len(), 2);

        // the commented-out [build] is the other way to deploy, and must parse too
        let built: String = example
            .lines()
            .filter(|line| !line.starts_with("image ="))
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        let built: Manifest = toml::from_str(&built).unwrap();
        assert!(built.image.is_none() && built.build.is_some());
    }

    #[test]
    fn starter_file_parses() {
        let name: Name = "web".parse().unwrap();
        let port = Port::try_from(8080).unwrap();
        let image: ImageRef = "nginx:alpine".parse().unwrap();
        let manifest: Manifest =
            toml::from_str(&starter(&name, &Starter::Image(&image), port)).unwrap();
        assert_eq!(manifest.name.as_str(), "web");
        assert_eq!(manifest.port, Some(port));
        assert_eq!(manifest.image, Some(image));
        assert_eq!(manifest.build, None);
        let manifest: Manifest = toml::from_str(&starter(&name, &Starter::Build, port)).unwrap();
        assert_eq!(manifest.image, None);
        assert_eq!(
            manifest
                .build
                .and_then(|b| b.dockerfile)
                .map(|d| d.to_string()),
            Some("Dockerfile".to_owned())
        );
    }

    #[test]
    fn reads_a_full_manifest() {
        let manifest: Manifest = toml::from_str(
            r#"
name = "web"
image = "ghcr.io/me/web:1"
port = 3000
domains = ["app.example.com", "www.example.com"]
health = "/up"
health_timeout = 90
command = ["node", "server.js"]
memory = "512m"
cpus = 0.5
replicas = 3
[env]
DATABASE_URL = "${{pg.DATABASE_URL}}"
[[volumes]]
name = "data"
path = "/data"
[backup]
every = "6h"
"#,
        )
        .unwrap();
        let request = manifest.into_request("unused:1".parse().unwrap());
        assert_eq!(request.domains.len(), 2);
        assert_eq!(
            request.memory.map(bird_core::MemoryLimit::mebibytes),
            Some(512)
        );
        assert_eq!(request.cpus.map(bird_core::CpuLimit::millicores), Some(500));
        assert_eq!(request.volumes.len(), 1);
        let backup = request.backup.unwrap();
        assert_eq!((backup.every.secs(), backup.keep.count()), (21_600, 7));
        assert_eq!(request.replicas.map(bird_core::Replicas::get), Some(3));
        assert_eq!(
            request.health.map(|h| h.to_string()).as_deref(),
            Some("/up")
        );
        assert_eq!(
            request.health_timeout.map(bird_core::HealthTimeout::secs),
            Some(90)
        );
    }

    #[test]
    fn reads_cron_jobs() {
        let manifest: Manifest = toml::from_str(
            r#"
name = "web"
image = "nginx"
[[cron]]
name = "report"
schedule = "0 6 * * MON"
command = ["bin/report"]
timeout = "30m"
[[cron]]
name = "cleanup"
schedule = "@hourly"
command = ["bin/cleanup", "--old"]
timeout = 90
[[cron]]
name = "ping"
schedule = "*/5 * * * *"
command = ["true"]
"#,
        )
        .unwrap();
        let timeouts: Vec<Option<u32>> = manifest
            .cron
            .iter()
            .map(|job| job.timeout.map(bird_core::CronTimeout::secs))
            .collect();
        assert_eq!(timeouts, [Some(1800), Some(90), None]);
        assert_eq!(manifest.cron[1].command.args(), ["bin/cleanup", "--old"]);
    }

    #[test]
    fn rejects_typos_and_bad_values() {
        let base = "name = \"web\"\nimage = \"nginx\"\n";
        assert!(toml::from_str::<Manifest>(base).is_ok());
        for bad in [
            "memroy = \"1g\"",
            "memory = \"1t\"",
            "cpus = 0.01",
            "port = 0",
            "health = \"grpc\"",
            "health = \"healthz\"",
            "health_timeout = \"1h\"",
            "[backup]\nevery = \"10m\"",
            "[backup]\nkeep = 3",
            "[backup]\nevery = \"1d\"\nevry = 1",
            "replicas = 0",
            "replicas = 99",
            "[[cron]]\nname = \"a\"\nschedule = \"61 * * * *\"\ncommand = [\"true\"]",
            "[[cron]]\nname = \"a\"\nschedule = \"@daily\"\ncommand = []",
            "[[cron]]\nname = \"a\"\nschedule = \"@daily\"\ncommand = [\"true\"]\ntimeout = \"2d\"",
            "[[cron]]\nname = \"a\"\nschedule = \"@daily\"\ncommand = [\"true\"]\nevery = 1",
            "[[cron]]\nname = \"A\"\nschedule = \"@daily\"\ncommand = [\"true\"]",
        ] {
            assert!(
                toml::from_str::<Manifest>(&format!("{base}{bad}\n")).is_err(),
                "{bad}"
            );
        }
    }
}
