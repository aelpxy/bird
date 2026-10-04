use std::io;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use http_body::{Body, Frame};
use http_body_util::BodyExt;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf};

use super::{Adapter, Archive, BoxError};
use crate::{Error, Result};

const CHUNK_BYTES: usize = 64 * 1024;

// archives as files under a directory on this server, readable only by birdd's user
pub(crate) struct LocalDir {
    root: PathBuf,
    durable: bool,
}

impl LocalDir {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            durable: true,
        }
    }

    // for copies that are thrown away after a crash: skipping fsync keeps a paused service's
    // pause short on a busy disk
    pub(crate) fn scratch(root: PathBuf) -> Self {
        Self {
            root,
            durable: false,
        }
    }

    fn path(&self, key: &str) -> Result<PathBuf> {
        let relative = Path::new(key);
        if !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid backup key {key:?}"),
            )));
        }
        Ok(self.root.join(relative))
    }
}

impl Adapter for LocalDir {
    fn name(&self) -> &'static str {
        "local"
    }

    async fn put<B>(&self, key: &str, mut archive: B) -> Result<u64>
    where
        B: Body<Data = Bytes> + Send + Unpin,
        B::Error: Into<BoxError>,
    {
        let path = self.path(key)?;
        if let Some(dir) = path.parent() {
            tokio::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .await?;
        }
        // written under another name and renamed when complete, so a crash never leaves half an archive
        let partial = path.with_extension("partial");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&partial)
            .await?;
        let mut size: u64 = 0;
        let written = async {
            while let Some(frame) = archive.frame().await {
                let frame = frame.map_err(|err| io::Error::other(err.into()))?;
                if let Ok(data) = frame.into_data() {
                    file.write_all(&data).await?;
                    size = size.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                }
            }
            if self.durable {
                file.sync_all().await
            } else {
                file.flush().await
            }
        }
        .await;
        if let Err(err) = written {
            if let Err(cleanup) = tokio::fs::remove_file(&partial).await {
                tracing::warn!(path = %partial.display(), error = %cleanup, "could not remove partial backup");
            }
            return Err(err.into());
        }
        tokio::fs::rename(&partial, &path).await?;
        Ok(size)
    }

    async fn get(&self, key: &str) -> Result<Archive> {
        let path = self.path(key)?;
        match File::open(&path).await {
            Ok(file) => Ok(FileBody { file }.map_err(Into::into).boxed_unsync()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err(Error::BackupDataMissing(key.to_owned()))
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(tokio::fs::try_exists(self.path(key)?).await?)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.path(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        // the backup's directory goes once its last archive is gone; a non-empty one stays
        if let Some(dir) = path.parent().filter(|dir| *dir != self.root) {
            let _ = tokio::fs::remove_dir(dir).await;
        }
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut entries = match tokio::fs::read_dir(self.path(prefix)?).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let mut keys = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if entry.file_type().await?.is_file() && !name.ends_with(".partial") {
                keys.push(format!("{prefix}/{name}"));
            }
        }
        Ok(keys)
    }
}

struct FileBody {
    file: File,
}

impl Body for FileBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = self.get_mut();
        let mut chunk = vec![0; CHUNK_BYTES];
        let mut buffer = ReadBuf::new(&mut chunk);
        ready!(Pin::new(&mut this.file).poll_read(cx, &mut buffer))?;
        let filled = buffer.filled().len();
        if filled == 0 {
            return Poll::Ready(None);
        }
        chunk.truncate(filled);
        Poll::Ready(Some(Ok(Frame::data(Bytes::from(chunk)))))
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use http_body_util::Full;

    use super::*;

    fn storage() -> (LocalDir, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("bird-backups-{}", bird_core::BackupId::generate()));
        (LocalDir::new(root.clone()), root)
    }

    #[tokio::test]
    async fn stores_reads_and_deletes_archives() {
        let (storage, root) = storage();
        let content = vec![7_u8; CHUNK_BYTES * 2 + 5];
        let size = storage
            .put("b1/data.tar", Full::new(Bytes::from(content.clone())))
            .await
            .unwrap();
        assert_eq!(size, u64::try_from(content.len()).unwrap());
        assert!(storage.exists("b1/data.tar").await.unwrap());
        let mode = std::fs::metadata(root.join("b1/data.tar"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);

        let read = storage
            .get("b1/data.tar")
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(read.to_bytes(), content);

        storage.delete("b1/data.tar").await.unwrap();
        storage.delete("b1/data.tar").await.unwrap();
        assert!(!storage.exists("b1/data.tar").await.unwrap());
        assert!(!root.join("b1").exists());
        assert!(matches!(
            storage.get("b1/data.tar").await.unwrap_err(),
            Error::BackupDataMissing(_)
        ));
        std::fs::remove_dir_all(root).ok();
    }

    struct Broken;

    impl Body for Broken {
        type Data = Bytes;
        type Error = io::Error;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
            Poll::Ready(Some(Err(io::Error::other("export broke"))))
        }
    }

    #[tokio::test]
    async fn a_failed_write_leaves_nothing() {
        let (storage, root) = storage();
        assert!(storage.put("b2/data.tar", Broken).await.is_err());
        assert!(!storage.exists("b2/data.tar").await.unwrap());
        assert!(!root.join("b2/data.partial").exists());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn lists_complete_archives_under_a_prefix() {
        let (storage, root) = storage();
        assert_eq!(
            storage.list("database").await.unwrap(),
            Vec::<String>::new()
        );
        for key in ["database/bird-1.db", "database/bird-2.db", "b1/data.tar"] {
            storage.put(key, Full::new(Bytes::from("x"))).await.unwrap();
        }
        std::fs::write(root.join("database/bird-3.partial"), "half").unwrap();
        let mut keys = storage.list("database").await.unwrap();
        keys.sort();
        assert_eq!(keys, ["database/bird-1.db", "database/bird-2.db"]);
        assert!(storage.list("../outside").await.is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn refuses_keys_that_leave_the_root() {
        let (storage, _) = storage();
        for key in ["../escape.tar", "/etc/passwd", "a/../../b"] {
            assert!(storage.exists(key).await.is_err(), "{key}");
        }
    }
}
