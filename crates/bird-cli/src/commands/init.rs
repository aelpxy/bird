use std::fs::OpenOptions;
use std::io::Write;

use anyhow::{Context, Result};
use bird_api::MANIFEST_FILE;
use bird_core::{ImageRef, Name, Port};

use crate::manifest::starter;

pub(crate) fn run(name: &Name, image: &ImageRef, port: Port) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(MANIFEST_FILE)
        .with_context(|| format!("cannot create {MANIFEST_FILE}, does it already exist?"))?;
    file.write_all(starter(name.as_str(), image.as_str(), port.get()).as_bytes())?;
    println!("wrote {MANIFEST_FILE}, deploy it with `bird deploy`");
    Ok(())
}
