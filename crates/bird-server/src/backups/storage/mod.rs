mod local;

use bytes::Bytes;
use http_body::Body;
use http_body_util::combinators::UnsyncBoxBody;

pub(crate) use local::LocalDir;

use crate::Result;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub(crate) type Archive = UnsyncBoxBody<Bytes, BoxError>;

// what a place that holds backup archives must do; keys look like `<backup id>/<volume>.tar`
pub(crate) trait Adapter {
    // recorded with each backup, so a restore finds the storage that holds it
    fn name(&self) -> &'static str;

    // streams the archive in and returns its size; a failed write leaves nothing behind
    async fn put<B>(&self, key: &str, archive: B) -> Result<u64>
    where
        B: Body<Data = Bytes> + Send + Unpin,
        B::Error: Into<BoxError>;

    async fn get(&self, key: &str) -> Result<Archive>;

    async fn exists(&self, key: &str) -> Result<bool>;

    // deleting a key that is already gone succeeds
    async fn delete(&self, key: &str) -> Result<()>;

    // keys of complete archives directly under `prefix`, in no particular order
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

// the storage birdd was started with; a new adapter is a new variant
pub(crate) enum BackupStorage {
    Local(LocalDir),
}

impl Adapter for BackupStorage {
    fn name(&self) -> &'static str {
        match self {
            Self::Local(local) => local.name(),
        }
    }

    async fn put<B>(&self, key: &str, archive: B) -> Result<u64>
    where
        B: Body<Data = Bytes> + Send + Unpin,
        B::Error: Into<BoxError>,
    {
        match self {
            Self::Local(local) => local.put(key, archive).await,
        }
    }

    async fn get(&self, key: &str) -> Result<Archive> {
        match self {
            Self::Local(local) => local.get(key).await,
        }
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        match self {
            Self::Local(local) => local.exists(key).await,
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        match self {
            Self::Local(local) => local.delete(key).await,
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        match self {
            Self::Local(local) => local.list(prefix).await,
        }
    }
}
