use std::collections::BTreeMap;

use bird_podman::{Error, Podman, default_socket};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};

fn podman() -> Podman {
    Podman::new(default_socket())
}

fn archive(files: &[(&str, &str, u64)]) -> Full<Bytes> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, content, owner) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o640);
        header.set_uid(*owner);
        header.set_gid(*owner);
        header.set_cksum();
        builder
            .append_data(&mut header, path, content.as_bytes())
            .expect("writing to a Vec cannot fail");
    }
    Full::new(Bytes::from(
        builder.into_inner().expect("writing to a Vec cannot fail"),
    ))
}

async fn entries(podman: &Podman, volume: &str) -> Vec<(String, u64)> {
    let body = podman.export_volume(volume).await.expect("export starts");
    let tar = body.collect().await.expect("export finishes").to_bytes();
    let mut archive = tar::Archive::new(&tar[..]);
    let mut found: Vec<(String, u64)> = archive
        .entries()
        .expect("podman sends a tar")
        .map(|entry| entry.expect("readable entry"))
        .filter(|entry| entry.header().entry_type().is_file())
        .map(|entry| {
            let path = entry.path().expect("valid path").display().to_string();
            (path, entry.header().uid().expect("valid uid"))
        })
        .collect();
    found.sort();
    found
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn exports_what_was_imported_with_owners() {
    let podman = podman();
    let name = format!("bird-test-volume-{}", std::process::id());
    let labels = BTreeMap::from([("bird.test".to_owned(), name.clone())]);
    podman.ensure_volume(&name, &labels).await.unwrap();

    let files = [("a.txt", "hello\n", 0), ("db/data", "rows\n", 70)];
    podman.import_volume(&name, archive(&files)).await.unwrap();
    assert_eq!(
        entries(&podman, &name).await,
        [("a.txt".to_owned(), 0), ("db/data".to_owned(), 70)]
    );

    podman.remove_volume(&name).await.unwrap();
    assert!(matches!(
        podman.export_volume(&name).await.unwrap_err(),
        Error::NotFound { .. }
    ));
}
