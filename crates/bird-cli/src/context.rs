use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, bail};
use bird_core::BuildFile;
use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{Builder, HeaderMode};

use crate::dockerignore::DockerIgnore;

// matches birdd's limit, checked while packing so a huge context fails before it is uploaded
const MAX_CONTEXT_BYTES: usize = 512 * 1024 * 1024;
const IGNORE_FILE: &str = ".dockerignore";

pub(crate) struct Packed {
    pub(crate) archive: Vec<u8>,
    pub(crate) files: usize,
}

// a gzipped tar of the build context, without what .dockerignore excludes
pub(crate) fn pack(dir: &Path, dockerfile: &BuildFile) -> Result<Packed> {
    if !dir.join(dockerfile.as_str()).is_file() {
        bail!("no {dockerfile} in {}", dir.display());
    }
    let ignore = match fs::read_to_string(dir.join(IGNORE_FILE)) {
        Ok(text) => DockerIgnore::parse(&text),
        Err(err) if err.kind() == ErrorKind::NotFound => DockerIgnore::parse(""),
        Err(err) => return Err(err).context("cannot read .dockerignore"),
    };
    let mut packer = Packer {
        builder: Builder::new(GzEncoder::new(Vec::new(), Compression::default())),
        ignore,
        always: [dockerfile.as_str(), IGNORE_FILE],
        files: 0,
    };
    // fixed timestamps and owners, so unchanged files keep hitting the build cache
    packer.builder.mode(HeaderMode::Deterministic);
    packer.builder.follow_symlinks(false);
    packer.walk(dir, "")?;
    let files = packer.files;
    let archive = packer.builder.into_inner()?.finish()?;
    Ok(Packed { archive, files })
}

struct Packer<'a> {
    builder: Builder<GzEncoder<Vec<u8>>>,
    ignore: DockerIgnore,
    always: [&'a str; 2],
    files: usize,
}

impl Packer<'_> {
    fn walk(&mut self, root: &Path, relative: &str) -> Result<()> {
        let dir = root.join(relative);
        let mut entries = fs::read_dir(&dir)
            .with_context(|| format!("cannot read {}", dir.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                bail!("{} is not valid UTF-8", entry.path().display());
            };
            let path = if relative.is_empty() {
                name.to_owned()
            } else {
                format!("{relative}/{name}")
            };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if self.ignore.skips_dir(&path) {
                    continue;
                }
                self.builder.append_dir(&path, entry.path())?;
                self.walk(root, &path)?;
            } else if kind.is_file() || kind.is_symlink() {
                if self.ignore.excludes(&path) && !self.always.contains(&path.as_str()) {
                    continue;
                }
                self.builder
                    .append_path_with_name(entry.path(), &path)
                    .with_context(|| format!("cannot pack {path}"))?;
                self.files += 1;
                if self.builder.get_ref().get_ref().len() > MAX_CONTEXT_BYTES {
                    bail!(
                        "build context is over {} MiB, exclude more with .dockerignore",
                        MAX_CONTEXT_BYTES / 1024 / 1024
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use flate2::read::GzDecoder;

    use super::*;

    fn names(archive: &[u8]) -> Vec<String> {
        let mut tar = tar::Archive::new(GzDecoder::new(archive));
        tar.entries()
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                entry
                    .path()
                    .unwrap()
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn packs_without_ignored_files() {
        let dir = std::env::temp_dir().join(format!("bird-context-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        fs::write(dir.join("Dockerfile"), "FROM scratch\n").unwrap();
        fs::write(
            dir.join(".dockerignore"),
            "node_modules\n.dockerignore\n*.log\n",
        )
        .unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.join("node_modules/x/a.js"), "x").unwrap();
        fs::write(dir.join("debug.log"), "x").unwrap();

        let packed = pack(&dir, &"Dockerfile".parse().unwrap()).unwrap();
        assert_eq!(
            names(&packed.archive),
            [".dockerignore", "Dockerfile", "src", "src/main.rs"]
        );
        assert_eq!(packed.files, 3);
        assert!(pack(&dir, &"Missing".parse().unwrap()).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
