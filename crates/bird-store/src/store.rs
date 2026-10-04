use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Params, Row};

use crate::Result;
use crate::migrate::migrate;

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn transaction<T>(&mut self, work: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match work(self) {
            Ok(value) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(err) => {
                self.conn.execute_batch("ROLLBACK")?;
                Err(err)
            }
        }
    }

    // a consistent copy of the whole database while it stays in use; the target must not exist yet
    pub fn copy_to(&self, path: &Path) -> Result<()> {
        self.conn
            .execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
        Ok(())
    }

    pub(crate) fn execute(&self, sql: &str, params: impl Params) -> rusqlite::Result<usize> {
        self.conn.prepare_cached(sql)?.execute(params)
    }

    pub(crate) fn query_one<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: fn(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>> {
        Ok(self
            .conn
            .prepare_cached(sql)?
            .query_row(params, map)
            .optional()?)
    }

    pub(crate) fn query_all<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: fn(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params, map)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use bird_core::ProjectId;

    use super::*;
    use crate::testing::name;

    #[test]
    fn transaction_rolls_back_on_error() {
        let mut store = Store::open_in_memory().unwrap();
        let result = store.transaction(|store| {
            store.create_org(&name("first"))?;
            store.create_org(&name("first"))
        });
        assert!(matches!(result, Err(crate::Error::AlreadyExists("org"))));
        assert!(store.org_by_name(&name("first")).unwrap().is_none());

        store
            .transaction(|store| store.create_org(&name("second")))
            .unwrap();
        assert!(store.org_by_name(&name("second")).unwrap().is_some());
    }

    #[test]
    fn reopening_keeps_data() {
        let path = std::env::temp_dir().join(format!("bird-store-{}.db", ProjectId::generate()));
        {
            let store = Store::open(&path).unwrap();
            store.create_org(&name("default")).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert!(store.org_by_name(&name("default")).unwrap().is_some());
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn copies_a_live_database() {
        let dir = std::env::temp_dir().join(format!("bird-copy-{}", ProjectId::generate()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("bird.db")).unwrap();
        store.create_org(&name("default")).unwrap();
        let copy = dir.join("copy.db");
        store.copy_to(&copy).unwrap();
        assert!(store.copy_to(&copy).is_err());
        drop(store);
        let restored = Store::open(&copy).unwrap();
        assert!(restored.org_by_name(&name("default")).unwrap().is_some());
        drop(restored);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
