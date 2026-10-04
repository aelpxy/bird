use std::fmt::Display;
use std::io::IsTerminal;
use std::sync::OnceLock;

use bird_core::{DeploymentStatus, MachineState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paint {
    Bold,
    Dim,
    Red,
    Green,
    Yellow,
    Cyan,
}

impl Paint {
    const fn code(self) -> &'static str {
        match self {
            Self::Bold => "1",
            Self::Dim => "2",
            Self::Red => "31",
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Cyan => "36",
        }
    }
}

// each stream is checked on its own so `bird ls | less` stays plain while errors keep color
pub(crate) fn out(paint: Paint, text: impl Display) -> String {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    apply(
        *ENABLED.get_or_init(|| allowed() && std::io::stdout().is_terminal()),
        paint,
        text,
    )
}

pub(crate) fn err(paint: Paint, text: impl Display) -> String {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    apply(
        *ENABLED.get_or_init(|| allowed() && std::io::stderr().is_terminal()),
        paint,
        text,
    )
}

fn apply(enabled: bool, paint: Paint, text: impl Display) -> String {
    if enabled {
        format!("\x1b[{}m{text}\x1b[0m", paint.code())
    } else {
        text.to_string()
    }
}

// https://no-color.org
fn allowed() -> bool {
    std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
}

#[must_use]
pub(crate) const fn machine(state: MachineState) -> Paint {
    match state {
        MachineState::Running => Paint::Green,
        MachineState::Failed => Paint::Red,
        MachineState::Created | MachineState::Starting | MachineState::Stopping => Paint::Yellow,
        MachineState::Stopped | MachineState::Destroyed => Paint::Dim,
    }
}

#[must_use]
pub(crate) const fn deployment(status: DeploymentStatus) -> Paint {
    match status {
        DeploymentStatus::Active => Paint::Green,
        DeploymentStatus::Failed => Paint::Red,
        DeploymentStatus::Pending | DeploymentStatus::Deploying => Paint::Yellow,
        DeploymentStatus::Superseded => Paint::Dim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_only_when_enabled() {
        assert_eq!(apply(true, Paint::Green, "ok"), "\x1b[32mok\x1b[0m");
        assert_eq!(apply(false, Paint::Green, "ok"), "ok");
    }
}
