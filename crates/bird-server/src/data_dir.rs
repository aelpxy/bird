use std::fs::{self, DirBuilder, Permissions};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

use crate::Result;

const PRIVATE: u32 = 0o700;

// the directory holds secrets (env vars, api token, tls keys), so nobody else may enter it
pub(crate) fn prepare(path: &Path) -> Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE)
        .create(path)?;
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != PRIVATE {
        fs::set_permissions(path, Permissions::from_mode(PRIVATE))?;
        tracing::info!(path = %path.display(), from = format!("{mode:o}"), "restricted data directory to its owner");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bird_core::ProjectId;

    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn creates_private_directory() {
        let path = std::env::temp_dir().join(format!("bird-data-{}/nested", ProjectId::generate()));
        prepare(&path).unwrap();
        assert_eq!(mode(&path), PRIVATE);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn tightens_existing_directory() {
        let path = std::env::temp_dir().join(format!("bird-data-{}", ProjectId::generate()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();
        prepare(&path).unwrap();
        assert_eq!(mode(&path), PRIVATE);
        fs::remove_dir(path).unwrap();
    }
}
