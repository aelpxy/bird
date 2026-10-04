use std::io::Write;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::args::Args;

// rendered into memory first, clap_complete panics when writing to a closed pipe
pub(crate) fn run(shell: Shell) -> Result<()> {
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Args::command(), "bird", &mut script);
    std::io::stdout().write_all(&script)?;
    Ok(())
}
