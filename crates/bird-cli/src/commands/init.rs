use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use bird_api::MANIFEST_FILE;
use bird_core::{ImageRef, Name, Port};

use crate::manifest::{Starter, starter};
use crate::ui::style::{self, Paint};

const DOCKERFILE: &str = "Dockerfile";

pub(crate) fn run(name: Option<Name>, image: Option<&ImageRef>, port: Option<Port>) -> Result<()> {
    let dir = std::env::current_dir().context("cannot read the current directory")?;
    let name = match name {
        Some(name) => name,
        None => name_from_dir(&dir)?,
    };
    let dockerfile = match std::fs::read_to_string(DOCKERFILE) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).context("cannot read the Dockerfile"),
    };
    let source = match (image, &dockerfile) {
        (Some(image), _) => Starter::Image(image),
        (None, Some(_)) => Starter::Build,
        (None, None) => bail!(
            "no {DOCKERFILE} here to build, pass an image to run instead: bird init {name} <image>"
        ),
    };
    let port = port
        .or_else(|| dockerfile.as_deref().and_then(exposed_port))
        .unwrap_or(Port::HTTP);

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(MANIFEST_FILE)
        .map_err(|err| match err.kind() {
            ErrorKind::AlreadyExists => {
                anyhow::anyhow!("{MANIFEST_FILE} already exists here, deploy it with `bird deploy`")
            }
            _ => anyhow::Error::new(err).context(format!("cannot create {MANIFEST_FILE}")),
        })?;
    file.write_all(starter(&name, &source, port).as_bytes())?;
    let from = match source {
        Starter::Image(image) => format!("runs {image}"),
        Starter::Build => format!("builds ./{DOCKERFILE}"),
    };
    println!(
        "{} wrote {MANIFEST_FILE}: {name} {from} on port {port}",
        style::out(Paint::Green, "✓")
    );
    println!("  next: {}", style::out(Paint::Bold, "bird deploy"));
    Ok(())
}

// directory names often hold capitals, dots or underscores that service names cannot
fn name_from_dir(dir: &Path) -> Result<Name> {
    let raw = dir
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned
        .trim_start_matches(|c: char| c == '-' || c.is_ascii_digit())
        .trim_end_matches('-');
    trimmed.parse().map_err(|_| {
        anyhow::anyhow!(
            "cannot name a service after the directory {raw:?}, pass one: bird init <name>"
        )
    })
}

// the first EXPOSE is the port most images document their app on
fn exposed_port(dockerfile: &str) -> Option<Port> {
    dockerfile.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        let instruction = words.next()?;
        if !instruction.eq_ignore_ascii_case("EXPOSE") {
            return None;
        }
        let first = words.next()?;
        let number = first.split('/').next()?.parse::<u16>().ok()?;
        Port::try_from(number).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_services_after_directories() {
        let name = |dir: &str| name_from_dir(Path::new(dir)).map(|n| n.to_string());
        assert_eq!(name("/home/me/web").unwrap(), "web");
        assert_eq!(name("/src/My_App.v2").unwrap(), "my-app-v2");
        assert_eq!(name("/src/2048-game").unwrap(), "game");
        assert_eq!(name("/src/api2").unwrap(), "api2");
        assert!(name("/").is_err());
        assert!(name("/src/___").is_err());
    }

    #[test]
    fn reads_the_exposed_port() {
        let port = |text: &str| exposed_port(text).map(Port::get);
        assert_eq!(
            port("FROM node:22\nEXPOSE 3000\nCMD [\"node\"]"),
            Some(3000)
        );
        assert_eq!(port("from alpine\n  expose 8080/tcp 9090"), Some(8080));
        assert_eq!(port("FROM alpine\n# EXPOSE 1\n"), None);
        assert_eq!(port("EXPOSE $PORT"), None);
    }
}
