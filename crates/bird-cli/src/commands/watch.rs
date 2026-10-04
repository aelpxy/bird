use std::io::{IsTerminal, Write};
use std::time::Duration;

use anyhow::Result;
use bird_core::Name;

use super::history::unix_now;
use super::status::{describe, fetch};
use crate::client::ApiClient;
use crate::ui::Output;
use crate::ui::style::{self, Paint};
use crate::ui::terminal;

const INTERVAL: Duration = Duration::from_secs(2);
// no line wrapping and no cursor, so a frame always fits the screen and redraws in place
const ENTER: &str = "\x1b[?7l\x1b[?25l";
const LEAVE: &str = "\x1b[?7h\x1b[?25h";

// redrawn in place on a terminal; piped, each refresh is appended (one JSON line each with --json)
pub(super) async fn run(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    if out.json || !std::io::stdout().is_terminal() {
        return append(client, name, out).await;
    }
    let _screen = Screen::enter()?;
    tokio::select! {
        result = redraw(client, name) => result,
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}

async fn append(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    loop {
        let service = fetch(client, name).await?;
        let frame = if out.json {
            serde_json::to_string(&service)?
        } else {
            describe(&service, unix_now())
        };
        show(&format!("{frame}\n"))?;
        tokio::time::sleep(INTERVAL).await;
    }
}

// errors are shown in place and retried, so a birdd restart or a deploy does not end the watch
async fn redraw(client: &ApiClient, name: &Name) -> Result<()> {
    let hint = style::out(
        Paint::Dim,
        format!("every {}s, ctrl-c to stop", INTERVAL.as_secs()),
    );
    loop {
        let body = match fetch(client, name).await {
            Ok(service) => describe(&service, unix_now()),
            Err(err) => format!("{} {err:#}\n", style::out(Paint::Red, "error:")),
        };
        let (_, rows) = terminal::size();
        show(&repaint(&format!("{hint}\n\n{body}"), rows))?;
        tokio::time::sleep(INTERVAL).await;
    }
}

fn show(text: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()
}

// overwrites the previous frame line by line instead of clearing first, so it does not flicker;
// the last row stays free, since a newline there would scroll the frame up
fn repaint(frame: &str, rows: u16) -> String {
    let mut text = String::from("\x1b[H");
    for line in frame.lines().take(usize::from(rows.saturating_sub(1))) {
        text.push_str(line);
        text.push_str("\x1b[K\n");
    }
    text.push_str("\x1b[J");
    text
}

struct Screen;

impl Screen {
    fn enter() -> std::io::Result<Self> {
        show(ENTER)?;
        Ok(Self)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // nothing is left to report to once the watch ends, and the terminal may already be gone
        let _ = show(LEAVE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repaints_over_the_previous_frame() {
        assert_eq!(repaint("a\nb\n", 24), "\x1b[Ha\x1b[K\nb\x1b[K\n\x1b[J");
    }

    #[test]
    fn keeps_the_frame_within_the_screen() {
        assert_eq!(repaint("a\nb\nc\n", 3), "\x1b[Ha\x1b[K\nb\x1b[K\n\x1b[J");
        assert_eq!(repaint("a\n", 0), "\x1b[H\x1b[J");
    }
}
