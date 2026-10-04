use std::io::IsTerminal;

use anyhow::{Context, Result};
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcgetwinsize, tcsetattr};

const FALLBACK_SIZE: (u16, u16) = (80, 24);

// a terminal on both ends is what makes a session interactive; pipes and files keep plain streaming
#[must_use]
pub(crate) fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

// columns and rows of the terminal bird runs in
#[must_use]
pub(crate) fn size() -> (u16, u16) {
    tcgetwinsize(std::io::stdout())
        .ok()
        .filter(|size| size.ws_col > 0 && size.ws_row > 0)
        .map_or(FALLBACK_SIZE, |size| (size.ws_col, size.ws_row))
}

// keystrokes go to the remote program unprocessed, Ctrl-C included; dropping it puts the terminal back
pub(crate) struct RawMode {
    original: Termios,
}

impl RawMode {
    pub(crate) fn enable() -> Result<Self> {
        let stdin = std::io::stdin();
        let original = tcgetattr(&stdin).context("cannot read the terminal settings")?;
        let mut raw = original.clone();
        raw.make_raw();
        tcsetattr(&stdin, OptionalActions::Now, &raw)
            .context("cannot switch the terminal to raw mode")?;
        Ok(Self { original })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = tcsetattr(std::io::stdin(), OptionalActions::Now, &self.original);
    }
}
