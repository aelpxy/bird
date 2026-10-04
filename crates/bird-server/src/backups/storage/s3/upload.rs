use bytes::Bytes;
use object_store::{MultipartUpload, WriteMultipart};

use super::within;
use crate::{Error, Result};

// s3 allows 10,000 parts, so one archive may be up to about 156 GiB
pub(super) const PART_BYTES: usize = 16 * 1024 * 1024;
const PARTS_IN_FLIGHT: usize = 2;

// a multipart upload that is aborted however it ends early, also when its future is dropped, like
// a backup cut short by shutdown; otherwise the bucket keeps its parts, and charges for them
pub(super) struct Upload {
    writer: Option<WriteMultipart>,
    key: String,
}

impl Upload {
    pub(super) fn new(started: Box<dyn MultipartUpload>, key: &str) -> Self {
        Self {
            writer: Some(WriteMultipart::new_with_chunk_size(started, PART_BYTES)),
            key: key.to_owned(),
        }
    }

    pub(super) async fn write(&mut self, data: Bytes) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(ended)?;
        within(
            super::STALL_TIMEOUT,
            writer.wait_for_capacity(PARTS_IN_FLIGHT),
        )
        .await?;
        writer.put(data);
        Ok(())
    }

    // a failed completion aborts the upload itself
    pub(super) async fn finish(mut self) -> Result<()> {
        let writer = self.writer.take().ok_or_else(ended)?;
        within(super::STALL_TIMEOUT, writer.finish()).await?;
        Ok(())
    }

    pub(super) async fn abort(mut self) {
        if let Some(writer) = self.writer.take() {
            abort(writer, &self.key).await;
        }
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        let Some(writer) = self.writer.take() else {
            return;
        };
        let key = std::mem::take(&mut self.key);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move { abort(writer, &key).await });
        } else {
            tracing::warn!(key, "a cancelled backup upload was left unfinished");
        }
    }
}

async fn abort(writer: WriteMultipart, key: &str) {
    match writer.abort().await {
        Ok(()) => tracing::debug!(key, "aborted an unfinished backup upload"),
        Err(err) => {
            tracing::warn!(key, error = %err, "could not abort an unfinished backup upload");
        }
    }
}

fn ended() -> Error {
    Error::BackupFailed("the upload already ended".to_owned())
}
