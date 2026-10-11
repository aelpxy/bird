// Embeds the built dashboard (`frontend/dist`) into birdd as a table of paths and bytes.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let frontend = manifest.join("../../frontend");
    let dist = frontend.join("dist");
    let mut files = Vec::new();
    if dist.is_dir() {
        println!("cargo:rerun-if-changed={}", dist.display());
        collect(&dist, &mut files)?;
    } else {
        // a missing path would rerun this on every build, the folder notices dist appearing
        println!("cargo:rerun-if-changed={}", frontend.display());
    }
    files.sort();
    let mut table = String::from("pub(crate) static FILES: &[(&str, &[u8])] = &[\n");
    for file in &files {
        let (Ok(relative), Some(absolute)) = (file.strip_prefix(&dist), file.to_str()) else {
            continue;
        };
        let Some(relative) = relative.to_str() else {
            println!(
                "cargo:warning=skipping non-utf-8 dashboard file {}",
                file.display()
            );
            continue;
        };
        writeln!(
            table,
            "    ({:?}, include_bytes!({absolute:?})),",
            format!("/{relative}")
        )
        .map_err(io::Error::other)?;
    }
    table.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default());
    fs::write(out.join("dashboard.rs"), table)
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
