use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 4096;

// the program and arguments a container starts with, replacing the image's default command
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = Vec<String>))]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Command(Vec<String>);

impl Command {
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.0
    }
}

impl TryFrom<Vec<String>> for Command {
    type Error = ValidationError;

    fn try_from(args: Vec<String>) -> Result<Self, Self::Error> {
        let valid = !args.is_empty()
            && args.len() <= MAX_ARGS
            && args.first().is_some_and(|program| !program.is_empty())
            && args
                .iter()
                .all(|arg| arg.len() <= MAX_ARG_BYTES && !arg.contains('\0'));
        if valid {
            Ok(Self(args))
        } else {
            Err(ValidationError::Command)
        }
    }
}

impl From<Command> for Vec<String> {
    fn from(command: Command) -> Self {
        command.0
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(args: &[&str]) -> Result<Command, ValidationError> {
        Command::try_from(args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn accepts_programs_with_arguments() {
        let ok = command(&["sh", "-c", "exec valkey-server --appendonly yes"]).unwrap();
        assert_eq!(ok.args().len(), 3);
    }

    #[test]
    fn rejects_empty_or_unsafe_commands() {
        assert!(command(&[]).is_err());
        assert!(command(&[""]).is_err());
        assert!(command(&["sh", "a\0b"]).is_err());
        assert!(command(&["x"; 65]).is_err());
    }
}
