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
        // the s3 gateway starts after the filer, and podman's port forwarder accepts connections
        // before anything listens behind it
        (0..60)
            .find(|_| {
                let up = server.answers_http();
                if !up {
                    std::thread::sleep(Duration::from_millis(500));
                }
                up
            })
            .expect("the s3 gateway answers within half a minute");
        server
    }

    fn answers_http(&self) -> bool {
        use std::io::{Read, Write};
        let address = self.endpoint.trim_start_matches("http://");
        let Ok(mut stream) = std::net::TcpStream::connect(address) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut reply = [0_u8; 5];
        stream.write_all(b"GET / HTTP/1.0\r\n\r\n").is_ok()
            && stream.read_exact(&mut reply).is_ok()
            && reply == *b"HTTP/"
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

    // uploads started but neither completed nor aborted; seaweedfs keeps them under .uploads
    fn unfinished_uploads(&self) -> String {
        podman(&[
            "exec",
            &self.name,
            "sh",
            "-c",
            "echo 'fs.ls /buckets/bird/.uploads' | weed shell",
        ])
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

// one part of data, then nothing ever again, like an export cut short when birdd stops
struct Stalls {
    sent: bool,
}

impl Body for Stalls {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<std::result::Result<Frame<Bytes>, io::Error>>> {
        if self.sent {
            return std::task::Poll::Pending;
        }
        self.sent = true;
        let part = Bytes::from(vec![7_u8; PART_BYTES + 1]);
        std::task::Poll::Ready(Some(Ok(Frame::data(part))))
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn a_cancelled_upload_leaves_no_parts() {
    let server = Server::start("bird-test-s3-cancelled");
    let storage = server.storage(None);
    let upload = storage.put("b3/data.tar", Stalls { sent: false });
    let cut = tokio::time::timeout(Duration::from_secs(5), upload).await;
    assert!(cut.is_err(), "the upload ended by itself: {cut:?}");

    // the abort runs in the background once the upload is dropped
    let mut left = server.unfinished_uploads();
    for _ in 0..20 {
        if left.trim().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        left = server.unfinished_uploads();
    }
    assert!(left.trim().is_empty(), "unfinished uploads: {left}");
    assert!(!storage.exists("b3/data.tar").await.unwrap());
}
