use anyhow::Result;
use serde::Serialize;

// results go to stdout and progress to stderr, so `--json` output pipes cleanly
#[derive(Debug, Clone, Copy)]
pub(crate) struct Output {
    pub(crate) json: bool,
}

impl Output {
    // prints the value when --json is set; returns whether it did so callers skip their own text
    pub(crate) fn json<T: Serialize>(self, value: &T) -> Result<bool> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        Ok(self.json)
    }
}
