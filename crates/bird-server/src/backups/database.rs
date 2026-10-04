use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bird_core::{BackupInterval, BackupKeep};
use bytes::Bytes;
use http_body_util::Full;

use super::storage::Adapter;
use crate::Result;
use crate::state::AppState;

// listed from storage rather than recorded in bird.db, so they are still found when bird.db is lost
const PREFIX: &str = "database";
const RETRY_AFTER: Duration = Duration::from_mins(15);

pub(crate) struct DatabaseBackups {
    every: BackupInterval,
    keep: BackupKeep,
    // the copy is written here first, inside the private data directory
    scratch: PathBuf,
    failed_at: Option<Instant>,
}

impl DatabaseBackups {
    pub(crate) fn new(every: BackupInterval, keep: BackupKeep, scratch: PathBuf) -> Self {
        Self {
            every,
            keep,
            scratch,
            failed_at: None,
        }
    }

    pub(crate) async fn back_up_if_due(&mut self, state: &AppState) {
        if self.failed_at.is_some_and(|at| at.elapsed() < RETRY_AFTER) {
            return;
        }
        let outcome = async {
            let keys = state.backups.list(PREFIX).await?;
            if !is_due(&keys, self.every, unix_millis()) {
                return Ok(None);
            }
            let key = self.back_up(state).await?;
            self.prune(state, keys).await;
            Ok::<_, crate::Error>(Some(key))
        }
        .await;
        match outcome {
            Ok(Some(key)) => {
                self.failed_at = None;
                tracing::info!(key, "bird.db backed up");
            }
            Ok(None) => {}
            Err(err) => {
                self.failed_at = Some(Instant::now());
                tracing::error!(error = %err, "could not back up bird.db");
            }
        }
    }

    async fn back_up(&self, state: &AppState) -> Result<String> {
        if tokio::fs::try_exists(&self.scratch).await? {
            tokio::fs::remove_file(&self.scratch).await?;
        }
        let scratch = self.scratch.clone();
        // VACUUM INTO writes a consistent copy while birdd keeps using the database
        state.db.call(move |store| store.copy_to(&scratch)).await?;
        let copy = tokio::fs::read(&self.scratch).await;
        if let Err(err) = tokio::fs::remove_file(&self.scratch).await {
            tracing::warn!(path = %self.scratch.display(), error = %err, "could not remove the bird.db copy");
        }
        let key = format!("{PREFIX}/bird-{}.db", unix_millis());
        state
            .backups
            .put(&key, Full::new(Bytes::from(copy?)))
            .await?;
        Ok(key)
    }

    async fn prune(&self, state: &AppState, mut older: Vec<String>) {
        older.sort_by_key(|key| std::cmp::Reverse(taken_at(key)));
        // the copy just made is not in the list yet, so one fewer of the older ones stays
        for key in older.iter().skip(self.keep.count().saturating_sub(1)) {
            if let Err(err) = state.backups.delete(key).await {
                tracing::warn!(key, error = %err, "could not delete an old bird.db backup");
            }
        }
    }
}

fn taken_at(key: &str) -> Option<u128> {
    key.strip_prefix(PREFIX)?
        .strip_prefix("/bird-")?
        .strip_suffix(".db")?
        .parse()
        .ok()
}

fn is_due(keys: &[String], every: BackupInterval, now_millis: u128) -> bool {
    let every = every.duration().as_millis();
    keys.iter()
        .filter_map(|key| taken_at(key))
        .max()
        .is_none_or(|newest| now_millis.saturating_sub(newest) >= every)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_time_from_keys() {
        assert_eq!(
            taken_at("database/bird-1791079239000.db"),
            Some(1_791_079_239_000)
        );
        assert_eq!(taken_at("database/notes.txt"), None);
        assert_eq!(taken_at("01a1/data.tar"), None);
    }

    #[test]
    fn due_when_the_newest_copy_is_old_enough() {
        let every: BackupInterval = "1d".parse().unwrap();
        let day = 86_400_000;
        assert!(is_due(&[], every, day));
        let keys = vec![
            "database/bird-1000.db".to_owned(),
            format!("database/bird-{}.db", 1000 + day),
        ];
        assert!(!is_due(&keys, every, 1000 + day + day - 1));
        assert!(is_due(&keys, every, 1000 + day + day));
    }
}
