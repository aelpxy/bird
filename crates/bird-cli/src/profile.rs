use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_API: &str = "127.0.0.1:7070";
pub(crate) const TOKEN_ENV: &str = "BIRD_TOKEN";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) api: String,
    pub(crate) token: String,
}

pub(crate) struct Target {
    pub(crate) api: String,
    pub(crate) token: Option<String>,
}

pub(crate) fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|base| base.join("bird/config.json"))
}

pub(crate) fn load(path: &Path) -> Result<Option<Profile>> {
    match fs::read(path) {
        Ok(raw) => serde_json::from_slice(&raw)
            .map(Some)
            .with_context(|| format!("{} is not a valid bird config", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("cannot read {}", path.display())),
    }
}

pub(crate) fn save(path: &Path, profile: &Profile) -> Result<()> {
    if let Some(dir) = path.parent() {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    let staging = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&staging)?;
    file.write_all(&serde_json::to_vec_pretty(profile)?)?;
    file.sync_all()?;
    fs::rename(&staging, path)?;
    Ok(())
}

// a saved token is only sent to the server it was saved for
pub(crate) fn resolve(
    api_override: Option<String>,
    env_token: Option<String>,
    saved: Option<Profile>,
) -> Target {
    let api = api_override
        .or_else(|| saved.as_ref().map(|p| p.api.clone()))
        .unwrap_or_else(|| DEFAULT_API.to_owned());
    let token = env_token.or_else(|| saved.filter(|p| p.api == api).map(|p| p.token));
    Target { api, token }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn profile(api: &str) -> Profile {
        Profile {
            api: api.to_owned(),
            token: "saved".to_owned(),
        }
    }

    #[test]
    fn resolves_saved_profile() {
        let target = resolve(None, None, Some(profile("srv:7070")));
        assert_eq!(target.api, "srv:7070");
        assert_eq!(target.token.as_deref(), Some("saved"));
    }

    #[test]
    fn never_sends_saved_token_to_another_server() {
        let target = resolve(
            Some("other:7070".to_owned()),
            None,
            Some(profile("srv:7070")),
        );
        assert_eq!(target.api, "other:7070");
        assert_eq!(target.token, None);
    }

    #[test]
    fn env_token_wins() {
        let target = resolve(None, Some("env".to_owned()), Some(profile("srv:7070")));
        assert_eq!(target.token.as_deref(), Some("env"));
    }

    #[test]
    fn defaults_to_local_api() {
        let target = resolve(None, None, None);
        assert_eq!(target.api, DEFAULT_API);
        assert_eq!(target.token, None);
    }

    #[test]
    fn saves_privately_and_loads_back() {
        let dir = std::env::temp_dir().join(format!("bird-profile-{}", std::process::id()));
        let path = dir.join("bird/config.json");
        let saved = profile("srv:7070");
        save(&path, &saved).unwrap();
        assert!(load(&path).unwrap() == Some(saved));
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        fs::remove_dir_all(dir).unwrap();
    }
}
