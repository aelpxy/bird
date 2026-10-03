use rusqlite::Connection;

use crate::{Error, Result};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_machine_address.sql"),
    include_str!("../migrations/0003_tls.sql"),
    include_str!("../migrations/0004_replicas.sql"),
    include_str!("../migrations/0005_deployment_variables.sql"),
    include_str!("../migrations/0006_health.sql"),
    include_str!("../migrations/0007_volumes.sql"),
    include_str!("../migrations/0008_resolved_variables.sql"),
    include_str!("../migrations/0009_command.sql"),
    include_str!("../migrations/0010_resources.sql"),
    include_str!("../migrations/0011_registries.sql"),
];

pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let found: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let applied = usize::try_from(found).unwrap_or(usize::MAX);
    if applied > MIGRATIONS.len() {
        return Err(Error::SchemaTooNew {
            found,
            supported: MIGRATIONS.len(),
        });
    }
    for (version, sql) in (1_i64..).zip(MIGRATIONS).skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(usize::try_from(version).unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(
            migrate(&mut conn).unwrap_err(),
            Error::SchemaTooNew { found: 999, .. }
        ));
    }
}
