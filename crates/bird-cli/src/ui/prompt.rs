use std::fmt;
use std::io::{BufRead, IsTerminal, Write};

use anyhow::{Result, bail};
use bird_core::Name;

use super::style::{self, Paint};

// answering no is a choice, not a failure, so main reports it without the error styling
#[derive(Debug)]
pub(crate) struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

// destructive commands ask first; scripts without a terminal must pass --yes
pub(crate) fn confirm(question: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    let answer = ask(&format!("{question} {}", style::err(Paint::Dim, "[y/N] ")))?;
    if matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(Cancelled.into())
    }
}

// for losing data, a reflexive "y" is not enough
pub(crate) fn confirm_name(warning: &str, name: &Name, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    eprintln!("{}", style::err(Paint::Yellow, warning));
    let answer = ask(&format!(
        "type {} to confirm: ",
        style::err(Paint::Bold, name)
    ))?;
    if answer == name.as_str() {
        Ok(())
    } else {
        Err(Cancelled.into())
    }
}

fn ask(prompt: &str) -> Result<String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("this needs confirmation, pass --yes to run it without a terminal");
    }
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}
