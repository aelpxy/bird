use std::fmt::{self, Write as _};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use crate::{Error, Result};

const TOKEN_BYTES: usize = 32;
const MIN_TOKEN_LEN: usize = 32;

#[derive(Clone)]
pub(crate) struct ApiToken(Arc<str>);

impl ApiToken {
    pub(crate) fn load_or_create(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(raw) => {
                warn_if_exposed(path);
                Self::parse(raw.trim()).ok_or_else(|| Error::InvalidToken(path.to_path_buf()))
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Self::create(path),
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) fn matches(&self, candidate: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), candidate.as_bytes())
    }

    fn parse(raw: &str) -> Option<Self> {
        let valid = raw.len() >= MIN_TOKEN_LEN && raw.bytes().all(|b| b.is_ascii_graphic());
        valid.then(|| Self(raw.into()))
    }

    fn create(path: &Path) -> Result<Self> {
        let token = generate()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        tracing::info!(path = %path.display(), "generated api token");
        Ok(Self(token.into()))
    }
}

impl fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiToken(<redacted>)")
    }
}

fn generate() -> Result<String> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut token = String::with_capacity(TOKEN_BYTES * 2);
    for byte in bytes {
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

fn warn_if_exposed(path: &Path) {
    let exposed = std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0);
    if exposed {
        tracing::warn!(path = %path.display(), "api token file is readable by other users, run chmod 600 on it");
    }
}

// no early exit on the first mismatch so response timing does not leak the token
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use bird_core::ProjectId;

    use super::*;

    fn temp_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bird-token-{}", ProjectId::generate()))
    }

    #[test]
    fn creates_private_token_once() {
        let path = temp_path();
        let first = ApiToken::load_or_create(&path).unwrap();
        let second = ApiToken::load_or_create(&path).unwrap();
        assert_eq!(first.0, second.0);
        assert_eq!(first.0.len(), TOKEN_BYTES * 2);
        assert!(first.0.bytes().all(|b| b.is_ascii_hexdigit()));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_short_or_empty_token_files() {
        let path = temp_path();
        std::fs::write(&path, "short\n").unwrap();
        assert!(matches!(
            ApiToken::load_or_create(&path),
            Err(Error::InvalidToken(_))
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn matches_only_exact_token() {
        let token = ApiToken::parse(&"a".repeat(64)).unwrap();
        assert!(token.matches(&"a".repeat(64)));
        assert!(!token.matches(&"a".repeat(63)));
        assert!(!token.matches(&format!("{}b", "a".repeat(63))));
        assert!(!token.matches(""));
    }

    #[test]
    fn debug_hides_token() {
        let token = ApiToken::parse(&"s".repeat(64)).unwrap();
        assert!(!format!("{token:?}").contains("sss"));
    }
}
