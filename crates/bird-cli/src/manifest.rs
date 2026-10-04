use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bird_api::{MANIFEST_FILE, Manifest};
use bird_core::EnvKey;

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
    warn_about_secrets(&manifest, path);
    let dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    Ok(Some(Loaded { manifest, dir }))
}

// bird.toml is usually committed, so literal secrets in it end up in git history
fn warn_about_secrets(manifest: &Manifest, path: &Path) {
    let build_args = manifest.build.iter().flat_map(|build| &build.args);
    for (key, _) in build_args.filter(|(key, _)| looks_secret(key)) {
        eprintln!(
            "warning: build arg {key} in {} looks like a secret, build args stay readable in the image",
            path.display()
        );
    }
    for (key, value) in &manifest.env {
        if looks_secret(key) && !value.is_empty() && !value.contains("${{") {
            eprintln!(
                "warning: {key} in {} looks like a secret, set it with `bird env set {} {key}=...` instead",
                path.display(),
                manifest.name
            );
        }
    }
}

fn looks_secret(key: &EnvKey) -> bool {
    SECRET_HINTS.iter().any(|hint| key.as_str().contains(hint))
}

pub(crate) fn starter(name: &str, image: &str, port: u16) -> String {
    format!(
        r#"name = "{name}"
image = "{image}"
port = {port}
# domains = ["{name}.example.com"]
# health = "/healthz"
# health_timeout = "2m"
# memory = "512m"
# cpus = 0.5
# replicas = 2
# command = ["./server", "--listen", "0.0.0.0:{port}"]

# build from source on the server instead of pulling image; remove image above to use it
# [build]
# context = "."
# dockerfile = "Dockerfile"
# args = {{ NODE_ENV = "production" }}

# plain settings and references like ${{{{postgres.DATABASE_URL}}}}; set secrets with `bird env set`
[env]
# LOG_LEVEL = "info"

# [[volumes]]
# name = "data"
# path = "/data"
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_file_parses() {
        let manifest: Manifest = toml::from_str(&starter("web", "nginx:alpine", 8080)).unwrap();
        assert_eq!(manifest.name.as_str(), "web");
        assert_eq!(manifest.port.map(bird_core::Port::get), Some(8080));
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
            "replicas = 0",
            "replicas = 99",
        ] {
            assert!(
                toml::from_str::<Manifest>(&format!("{base}{bad}\n")).is_err(),
                "{bad}"
            );
        }
    }
}
