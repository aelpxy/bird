mod support;

use reqwest::StatusCode;
use serde_json::json;
use support::{Birdd, IMAGE, scope};

// the roles of one org, someone outside it, and what each may do; a dropped check here would let
// one org into another's services
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario reads best top to bottom"
)]
async fn org_roles_decide_what_users_may_do() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let owner = birdd.user("owner", "member").await;
    let member = birdd.user("member", "member").await;
    let outsider = birdd.user("outsider", "member").await;
    let org = birdd.project("team");
    let project = birdd.project("shop");
    let production = scope(&project, "production");

    assert_eq!(
        birdd
            .post(&root, "/v1/orgs", json!({ "name": org }))
            .await
            .status,
        StatusCode::CREATED
    );
    for (user, role) in [("owner", "owner"), ("member", "member")] {
        let set = birdd
            .put(
                &root,
                &format!("/v1/orgs/{org}/members/{user}"),
                json!({ "role": role }),
            )
            .await;
        assert_eq!(set.status, StatusCode::NO_CONTENT, "{:?}", set.body);
    }

    // the owner's only org is picked for the project
    let created = birdd
        .post(&owner, "/v1/projects", json!({ "name": project }))
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(created.body["org"], org);

    // members deploy, but neither create projects and environments nor manage people
    let deployed = birdd.deploy(&member, &production, "api", IMAGE).await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);
    let listed = birdd.get(&member, &format!("{production}/services")).await;
    assert_eq!(listed.body[0]["name"], "api");
    let environment = birdd
        .post(
            &member,
            &format!("/v1/projects/{project}/environments"),
            json!({ "name": "staging" }),
        )
        .await;
    assert_eq!(environment.status, StatusCode::FORBIDDEN);
    let another = birdd
        .post(
            &member,
            "/v1/projects",
            json!({ "name": birdd.project("x") }),
        )
        .await;
    assert_eq!(another.status, StatusCode::BAD_REQUEST);
    let invite = birdd
        .put(
            &member,
            &format!("/v1/orgs/{org}/members/outsider"),
            json!({ "role": "member" }),
        )
        .await;
    assert_eq!(invite.status, StatusCode::FORBIDDEN);
    let registry = birdd
        .put(
            &member,
            "/v1/registries/ghcr.io",
            json!({ "username": "u", "password": "p" }),
        )
        .await;
    assert_eq!(registry.status, StatusCode::FORBIDDEN);

    // to an outsider the project, its environments and the org do not exist
    for path in [
        format!("{production}/services"),
        format!("{production}/services/api"),
        format!("{}/services", scope(&project, "made-up")),
    ] {
        let probed = birdd.get(&outsider, &path).await;
        assert_eq!(probed.status, StatusCode::NOT_FOUND, "{path}");
        assert!(
            probed.body["error"]
                .as_str()
                .unwrap()
                .starts_with(&format!("project {project} not found"))
        );
    }
    assert_eq!(
        birdd
            .get(&outsider, &format!("/v1/orgs/{org}/members"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(birdd.get(&outsider, "/v1/projects").await.body, json!([]));
    assert_eq!(birdd.get(&outsider, "/v1/orgs").await.body, json!([]));
    let tokens = birdd.get(&outsider, "/v1/users/owner/tokens").await;
    assert_eq!(tokens.status, StatusCode::FORBIDDEN);

    // the last owner cannot leave, and removing a member takes their access at once
    let leave = birdd
        .delete(&owner, &format!("/v1/orgs/{org}/members/owner"))
        .await;
    assert_eq!(leave.status, StatusCode::CONFLICT);
    let removed = birdd
        .delete(&owner, &format!("/v1/orgs/{org}/members/member"))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(
        birdd
            .get(&member, &format!("{production}/services"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        birdd
            .delete(&root, &format!("{production}/services/api"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd
            .delete(&owner, &format!("/v1/projects/{project}"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    birdd.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn tokens_work_until_deleted_or_their_user_is_gone() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    assert_eq!(
        birdd.get("bird_not-a-token", "/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(birdd.get(&root, "/v1/me").await.body["name"], "root");

    let first = birdd.user("ada", "member").await;
    assert_eq!(
        birdd.get(&first, "/v1/me").await.body,
        json!({ "name": "ada", "role": "member", "has_password": false, "two_factor": false })
    );
    let issued = birdd
        .post(
            &first,
            "/v1/users/ada/tokens",
            json!({ "name": "ci", "expires_in_days": 1 }),
        )
        .await;
    assert_eq!(issued.status, StatusCode::CREATED);
    let ci = issued.body["token"].as_str().unwrap().to_owned();
    assert_eq!(birdd.get(&ci, "/v1/me").await.body["name"], "ada");
    assert_eq!(
        birdd.get(&first, "/v1/users").await.status,
        StatusCode::FORBIDDEN
    );

    let listed = birdd.get(&first, "/v1/users/ada/tokens").await.body;
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .all(|token| token.get("token").is_none())
    );
    assert_eq!(
        birdd.delete(&first, "/v1/users/ada/tokens/ci").await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd.get(&ci, "/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        birdd.delete(&root, "/v1/users/ada").await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd.get(&first, "/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    birdd.stop().await;
}
