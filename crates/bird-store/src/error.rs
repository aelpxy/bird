use rusqlite::{ErrorCode, ffi};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} already exists")]
    AlreadyExists(&'static str),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("parent of {0} not found")]
    ParentNotFound(&'static str),
    #[error("database schema version {found} is newer than supported version {supported}")]
    SchemaTooNew { found: i64, supported: usize },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not encode a stored value: {0}")]
    Encode(serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn write_error(entity: &'static str) -> impl Fn(rusqlite::Error) -> Error {
    move |err| match &err {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            match failure.extended_code {
                ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                    Error::AlreadyExists(entity)
                }
                ffi::SQLITE_CONSTRAINT_FOREIGNKEY => Error::ParentNotFound(entity),
                _ => Error::Sqlite(err),
            }
        }
        _ => Error::Sqlite(err),
    }
}

pub(crate) fn expect_changed(changed: usize, entity: &'static str) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound(entity))
    } else {
        Ok(())
    }
}
