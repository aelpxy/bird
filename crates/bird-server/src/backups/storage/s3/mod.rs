use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body::{Body, Frame};
use http_body_util::{BodyExt, StreamBody};
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::path::Path;
use object_store::prefix::PrefixStore;
use object_store::{ClientOptions, ObjectStore, ObjectStoreExt};

mod upload;

#[cfg(test)]
use upload::PART_BYTES;
use upload::Upload;

use super::{Adapter, Archive, BoxError};
use crate::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
// archives are large, so stalls are bounded instead of whole requests
const READ_TIMEOUT: Duration = Duration::from_mins(1);
const REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
const STALL_TIMEOUT: Duration = Duration::from_mins(10);

pub(crate) struct S3Settings {
    pub(crate) bucket: String,
    pub(crate) region: String,
    pub(crate) endpoint: Option<String>,
    pub(crate) prefix: Option<String>,
    pub(crate) access_key_id: String,
    pub(crate) secret_access_key: String,
}

// archives as objects in an s3-compatible bucket; an upload only shows up once it is complete
pub(crate) struct S3 {
    store: PrefixStore<AmazonS3>,
}

impl S3 {
    pub(crate) fn new(settings: S3Settings) -> Result<Self> {
        // reqwest is built without a provider of its own; an error means one is already installed
        let _ = rustls::crypto::ring::default_provider().install_default();
        let options = ClientOptions::new()
            .with_connect_timeout(CONNECT_TIMEOUT)
            .with_read_timeout(READ_TIMEOUT)
            .with_timeout_disabled();
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(settings.bucket)
            .with_region(settings.region)
            .with_access_key_id(settings.access_key_id)
            .with_secret_access_key(settings.secret_access_key)
            .with_client_options(options);
        if let Some(endpoint) = settings.endpoint {
            builder = builder
                .with_allow_http(endpoint.starts_with("http://"))
                .with_endpoint(endpoint);
        }
        let prefix = object_path(&settings.prefix.unwrap_or_default())?;
        Ok(Self {
            store: PrefixStore::new(builder.build()?, prefix),
        })
    }
}

impl Adapter for S3 {
    fn name(&self) -> &'static str {
        "s3"
    }

    async fn put<B>(&self, key: &str, mut archive: B) -> Result<u64>
    where
        B: Body<Data = Bytes> + Send + Unpin,
        B::Error: Into<BoxError>,
    {
        let path = object_path(key)?;
        let started = within(REQUEST_TIMEOUT, self.store.put_multipart(&path)).await?;
        let mut upload = Upload::new(started, key);
        let mut size: u64 = 0;
        let written = async {
            while let Some(frame) = archive.frame().await {
                let frame = frame.map_err(|err| std::io::Error::other(err.into()))?;
                if let Ok(data) = frame.into_data() {
                    size = size.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                    upload.write(data).await?;
                }
            }
            Ok::<_, Error>(())
        }
        .await;
        if let Err(err) = written {
            upload.abort().await;
            return Err(err);
        }
        upload.finish().await?;
        Ok(size)
    }

    async fn get(&self, key: &str) -> Result<Archive> {
        let path = object_path(key)?;
        match within(REQUEST_TIMEOUT, self.store.get(&path)).await {
            Ok(found) => {
                let frames = found
                    .into_stream()
                    .map_ok(Frame::data)
                    .map_err(BoxError::from);
                Ok(StreamBody::new(frames).boxed_unsync())
            }
            Err(Error::Storage(object_store::Error::NotFound { .. })) => {
                Err(Error::BackupDataMissing(key.to_owned()))
            }
            Err(err) => Err(err),
        }
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let path = object_path(key)?;
        match within(REQUEST_TIMEOUT, self.store.head(&path)).await {
            Ok(_) => Ok(true),
            Err(Error::Storage(object_store::Error::NotFound { .. })) => Ok(false),
            Err(err) => Err(err),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = object_path(key)?;
        match within(REQUEST_TIMEOUT, self.store.delete(&path)).await {
            Ok(()) | Err(Error::Storage(object_store::Error::NotFound { .. })) => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let path = object_path(prefix)?;
        let listed = within(REQUEST_TIMEOUT, self.store.list_with_delimiter(Some(&path))).await?;
        Ok(listed
            .objects
            .into_iter()
            .map(|object| object.location.to_string())
            .collect())
    }
}

async fn within<T>(
    limit: Duration,
    work: impl Future<Output = object_store::Result<T>>,
) -> Result<T> {
    tokio::time::timeout(limit, work)
        .await
        .map_err(|_| {
            Error::BackupFailed(format!("backup storage did not answer within {limit:?}"))
        })?
        .map_err(Error::from)
}

fn object_path(key: &str) -> Result<Path> {
    Path::parse(key).map_err(|err| Error::Storage(err.into()))
}

#[cfg(test)]
mod tests;
