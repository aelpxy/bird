use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body::{Body, Frame};
use http_body_util::{BodyExt, StreamBody};
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::path::Path;
use object_store::prefix::PrefixStore;
use object_store::{ClientOptions, ObjectStore, ObjectStoreExt, WriteMultipart};

use super::{Adapter, Archive, BoxError};
use crate::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
// archives are large, so stalls are bounded instead of whole requests
const READ_TIMEOUT: Duration = Duration::from_mins(1);
const REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
const STALL_TIMEOUT: Duration = Duration::from_mins(10);
// s3 allows 10,000 parts, so one archive may be up to about 156 GiB
const PART_BYTES: usize = 16 * 1024 * 1024;
const PARTS_IN_FLIGHT: usize = 2;

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
        let upload = within(REQUEST_TIMEOUT, self.store.put_multipart(&path)).await?;
        let mut writer = WriteMultipart::new_with_chunk_size(upload, PART_BYTES);
        let mut size: u64 = 0;
        let written = async {
            while let Some(frame) = archive.frame().await {
                let frame = frame.map_err(|err| std::io::Error::other(err.into()))?;
                if let Ok(data) = frame.into_data() {
                    within(STALL_TIMEOUT, writer.wait_for_capacity(PARTS_IN_FLIGHT)).await?;
                    size = size.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                    writer.put(data);
                }
            }
            Ok::<_, Error>(())
        }
        .await;
        if let Err(err) = written {
            if let Err(cleanup) = writer.abort().await {
                tracing::warn!(key, error = %cleanup, "could not abort partial backup upload");
            }
            return Err(err);
        }
        // a failed completion aborts the upload itself
        within(STALL_TIMEOUT, writer.finish()).await?;
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
mod tests {
    use std::io;
    use std::process::Command;

    use http_body_util::Full;

    use super::*;

    const IMAGE: &str = "docker.io/chrislusf/seaweedfs:latest";
    const ACCESS_KEY: &str = "birdtest";
    const SECRET_KEY: &str = "birdtestsecret";

    // a throwaway s3 server with bucket `bird`, which checks request signatures
    struct Server {
        name: String,
        endpoint: String,
    }

    impl Server {
        fn start(name: &str) -> Self {
            let dir = std::env::temp_dir().join(name);
            std::fs::create_dir_all(&dir).expect("temp dir is writable");
            let config = dir.join("s3.json");
            let identities = format!(
                r#"{{"identities":[{{"name":"bird","credentials":[{{"accessKey":"{ACCESS_KEY}","secretKey":"{SECRET_KEY}"}}],"actions":["Admin","Read","Write","List"]}}]}}"#
            );
            std::fs::write(&config, identities).expect("temp dir is writable");
            podman(&["rm", "-f", name]);
            let mount = format!("{}:/etc/s3.json:ro,Z", config.display());
            podman(&[
                "run",
                "-d",
                "--name",
                name,
                "-p",
                "127.0.0.1::8333",
                "-v",
                &mount,
                IMAGE,
                "server",
                "-s3",
                "-s3.config=/etc/s3.json",
            ]);
            let server = Self {
                name: name.to_owned(),
                endpoint: format!("http://{}", podman(&["port", name, "8333/tcp"]).trim()),
            };
            (0..60)
                .find(|_| {
                    let created = podman(&[
                        "exec",
                        name,
                        "sh",
                        "-c",
                        "echo 's3.bucket.create -name bird' | weed shell",
                    ]);
                    let up = created.contains("created bucket");
                    if !up {
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    up
                })
                .expect("seaweedfs comes up within a minute");
            server
        }

        fn settings(&self, prefix: Option<&str>) -> S3Settings {
            S3Settings {
                bucket: "bird".to_owned(),
                region: "us-east-1".to_owned(),
                endpoint: Some(self.endpoint.clone()),
                prefix: prefix.map(str::to_owned),
                access_key_id: ACCESS_KEY.to_owned(),
                secret_access_key: SECRET_KEY.to_owned(),
            }
        }

        fn storage(&self, prefix: Option<&str>) -> S3 {
            S3::new(self.settings(prefix)).expect("valid settings")
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            podman(&["rm", "-f", &self.name]);
        }
    }

    fn podman(args: &[&str]) -> String {
        let output = Command::new("podman")
            .args(args)
            .output()
            .expect("podman is installed");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    #[tokio::test]
    #[ignore = "requires a running podman socket"]
    async fn stores_reads_lists_and_deletes_objects() {
        let server = Server::start("bird-test-s3-objects");
        let storage = server.storage(Some("team/bird"));
        // more than two parts, and not a whole number of them
        let content: Vec<u8> = (0..=255_u8).cycle().take(PART_BYTES * 2 + 3).collect();
        let size = storage
            .put("b1/data.tar", Full::new(Bytes::from(content.clone())))
            .await
            .unwrap();
        assert_eq!(size, u64::try_from(content.len()).unwrap());
        storage
            .put("database/bird-1.db", Full::new(Bytes::from("db")))
            .await
            .unwrap();
        assert!(storage.exists("b1/data.tar").await.unwrap());

        let read = storage.get("b1/data.tar").await.unwrap();
        assert_eq!(read.collect().await.unwrap().to_bytes(), content);
        assert_eq!(storage.list("b1").await.unwrap(), ["b1/data.tar"]);
        assert_eq!(
            storage.list("database").await.unwrap(),
            ["database/bird-1.db"]
        );
        assert_eq!(storage.list("nothing").await.unwrap(), Vec::<String>::new());
        let whole_bucket = server.storage(None).list("team/bird/b1").await.unwrap();
        assert_eq!(whole_bucket, ["team/bird/b1/data.tar"]);

        storage.delete("b1/data.tar").await.unwrap();
        storage.delete("b1/data.tar").await.unwrap();
        assert!(!storage.exists("b1/data.tar").await.unwrap());
        assert!(matches!(
            storage.get("b1/data.tar").await.unwrap_err(),
            Error::BackupDataMissing(_)
        ));
        assert!(storage.exists("../escape.tar").await.is_err());
    }

    #[tokio::test]
    #[ignore = "requires a running podman socket"]
    async fn a_failed_upload_leaves_nothing() {
        let server = Server::start("bird-test-s3-failed");
        let storage = server.storage(None);
        let frames = futures_util::stream::iter([
            Ok(Frame::data(Bytes::from(vec![1_u8; PART_BYTES + 1]))),
            Err(io::Error::other("export broke")),
        ]);
        assert!(
            storage
                .put("b2/data.tar", StreamBody::new(frames))
                .await
                .is_err()
        );
        assert!(!storage.exists("b2/data.tar").await.unwrap());

        let wrong = S3::new(S3Settings {
            secret_access_key: "not-the-secret".to_owned(),
            ..server.settings(None)
        })
        .unwrap();
        assert!(matches!(
            wrong.exists("b2/data.tar").await,
            Err(Error::Storage(_))
        ));
    }
}
