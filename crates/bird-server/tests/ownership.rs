mod support;

use reqwest::StatusCode;
use serde_json::json;
use support::Birdd;

async fn add(birdd: &Birdd, org: &str, user: &str, role: &str) {
    let path = format!("/v1/orgs/{org}/members/{user}");
    let set = birdd.put(&birdd.root, &path, json!({ "role": role })).await;
    assert!(set.status == StatusCode::NO_CONTENT, "{:?}", set.body);
}

// an org's last owner stays until the org owns nothing, then the empty org goes with them
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn last_owners_stay_until_their_org_is_empty() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let olive = birdd.user("olive", "member").await;
    let mia = birdd.user("mia", "member").await;
    let org = birdd.project("team");
    let project = birdd.project("shop");
    assert_eq!(
        birdd
            .post(&root, "/v1/orgs", json!({ "name": org }))
            .await
            .status,
        StatusCode::CREATED
    );
    add(&birdd, &org, "olive", "owner").await;
    add(&birdd, &org, "mia", "member").await;
    let created = birdd
        .post(&olive, "/v1/projects", json!({ "name": project }))
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);

    // while the org owns a project, its last owner can neither leave nor be deleted
    let leave = birdd
        .delete(&olive, &format!("/v1/orgs/{org}/members/olive"))
        .await;
    assert_eq!(leave.status, StatusCode::CONFLICT);
    assert!(leave.body["error"].as_str().unwrap().contains(&project));
    assert_eq!(
        birdd.delete(&root, "/v1/users/olive").await.status,
        StatusCode::CONFLICT
    );

    // with a second owner, the first may leave, and the new last owner cannot step down
    add(&birdd, &org, "mia", "owner").await;
    assert_eq!(
        birdd
            .delete(&olive, &format!("/v1/orgs/{org}/members/olive"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let demote = birdd
        .put(
            &root,
            &format!("/v1/orgs/{org}/members/mia"),
            json!({ "role": "admin" }),
        )
        .await;
    assert_eq!(demote.status, StatusCode::CONFLICT);

    // once the org owns nothing, its last owner leaves and the org is gone
    assert_eq!(
        birdd
            .delete(&mia, &format!("/v1/projects/{project}"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd
            .delete(&mia, &format!("/v1/orgs/{org}/members/mia"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd
            .get(&root, &format!("/v1/orgs/{org}/members"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // deleting the last owner of an empty org deletes the org too
    let solo = birdd.project("solo");
    assert_eq!(
        birdd
            .post(&root, "/v1/orgs", json!({ "name": solo }))
            .await
            .status,
        StatusCode::CREATED
    );
    add(&birdd, &solo, "olive", "owner").await;
    assert_eq!(
        birdd.delete(&root, "/v1/users/olive").await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd
            .get(&root, &format!("/v1/orgs/{solo}/members"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    birdd.stop().await;
}

// a deleted starting org stays deleted across restarts, and projects go to the only org left
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn the_starting_org_is_only_created_on_an_empty_server() {
    let mut birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let acme = birdd.project("acme");
    for path in ["/v1/projects/default", "/v1/orgs/default"] {
        let removed = birdd.delete(&root, path).await;
        assert_eq!(
            removed.status,
            StatusCode::NO_CONTENT,
            "{path}: {:?}",
            removed.body
        );
    }
    let created = birdd.post(&root, "/v1/orgs", json!({ "name": acme })).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);

    birdd.restart().await;
    let orgs = birdd.get(&root, "/v1/orgs").await.body;
    assert_eq!(orgs.as_array().unwrap().len(), 1, "{orgs}");
    assert_eq!(orgs[0]["name"], acme.as_str());

    let shop = birdd.project("shop");
    let placed = birdd
        .post(&root, "/v1/projects", json!({ "name": shop }))
        .await;
    assert_eq!(placed.status, StatusCode::CREATED, "{:?}", placed.body);
    assert_eq!(placed.body["org"], acme.as_str());

    let beta = birdd.project("beta");
    birdd.post(&root, "/v1/orgs", json!({ "name": beta })).await;
    let unclear = birdd
        .post(
            &root,
            "/v1/projects",
            json!({ "name": birdd.project("blog") }),
        )
        .await;
    assert_eq!(
        unclear.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        unclear.body
    );
    assert!(
        unclear.body["error"]
            .as_str()
            .unwrap()
            .contains("name one with --org"),
        "{:?}",
        unclear.body
    );
    birdd.stop().await;
}
