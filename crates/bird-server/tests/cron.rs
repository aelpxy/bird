mod support;

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{Value, json};
use support::{Birdd, IMAGE, SHELL_IMAGE, scope};

const RUN_WAIT: Duration = Duration::from_mins(2);

async fn deploy_with_jobs(birdd: &Birdd, scope: &str, extra: Value) -> support::Response {
    let mut body =
        json!({ "name": "web", "image": SHELL_IMAGE, "port": 80, "env": { "GREETING": "hello" } });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    birdd
        .post(&birdd.root, &format!("{scope}/deploy"), body)
        .await
}

async fn trigger(birdd: &Birdd, service: &str, job: &str) -> support::Response {
    birdd
        .post(
            &birdd.root,
            &format!("{service}/cron/{job}/runs"),
            json!(null),
        )
        .await
}

// polls a run until it is no longer running
async fn finished(birdd: &Birdd, service: &str, job: &str, run: &Value) -> Value {
    let id = field(run, "id").as_str().expect("a run has an id");
    let path = format!("{service}/cron/{job}/runs/{id}");
    let deadline = tokio::time::Instant::now() + RUN_WAIT;
    loop {
        let found = birdd.get(&birdd.root, &path).await;
        assert_eq!(found.status, StatusCode::OK, "{:?}", found.body);
        if field(&found.body, "status") != "running" {
            return found.body;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{job} never finished"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn job_names(birdd: &Birdd, service: &str) -> Vec<String> {
    let listed = birdd.get(&birdd.root, &format!("{service}/cron")).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.body);
    listed
        .body
        .as_array()
        .expect("a list of jobs")
        .iter()
        .map(|job| field(job, "name").as_str().unwrap_or_default().to_owned())
        .collect()
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

async fn runs(birdd: &Birdd, service: &str, job: &str) -> Vec<Value> {
    let listed = birdd
        .get(&birdd.root, &format!("{service}/cron/{job}/runs"))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.body);
    listed.body.as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn manual_runs_record_output_exit_codes_and_time_limits() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let production = scope("default", "production");
    let service = format!("{production}/services/web");
    let api = birdd.deploy(&root, &production, "api", IMAGE).await;
    assert_eq!(api.status, StatusCode::OK, "{:?}", api.body);

    let deployed = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [
            { "name": "hello", "schedule": "0 3 * * *", "command": ["sh", "-c", "echo $GREETING from cron; echo warning >&2"] },
            { "name": "fails", "schedule": "0 3 * * *", "command": ["sh", "-c", "echo before; exit 3"] },
            { "name": "slow", "schedule": "0 3 * * *", "command": ["sleep", "60"], "timeout": "3s" },
            { "name": "peer", "schedule": "0 3 * * *", "command": ["wget", "-qO-", "http://api/"] }
        ] }),
    )
    .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);
    assert_eq!(
        job_names(&birdd, &service).await,
        ["fails", "hello", "peer", "slow"]
    );
    let listed = birdd.get(&root, &format!("{service}/cron")).await.body;
    assert_eq!(listed[0]["timeout"], 3600, "{listed}");
    assert_eq!(listed[3]["timeout"], 3, "{listed}");
    assert!(listed[0]["next_run_at"].is_i64(), "{listed}");
    assert!(listed[0]["last_run"].is_null(), "{listed}");

    let started = trigger(&birdd, &service, "hello").await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    assert_eq!(started.body["status"], "running");
    assert_eq!(started.body["trigger"], "manual");
    let hello = finished(&birdd, &service, "hello", &started.body).await;
    assert_eq!(hello["status"], "succeeded", "{hello}");
    assert_eq!(hello["exit_code"], 0);
    let output = hello["output"].as_str().unwrap_or_default();
    assert!(output.contains("hello from cron\n"), "{output}");
    assert!(output.contains("warning\n"), "{output}");

    let started = trigger(&birdd, &service, "fails").await;
    let fails = finished(&birdd, &service, "fails", &started.body).await;
    assert_eq!(fails["status"], "failed", "{fails}");
    assert_eq!(fails["exit_code"], 3);
    assert_eq!(fails["output"], "before\n");

    let started = trigger(&birdd, &service, "peer").await;
    let peer = finished(&birdd, &service, "peer", &started.body).await;
    assert_eq!(peer["status"], "succeeded", "{peer}");
    assert!(
        peer["output"]
            .as_str()
            .unwrap_or_default()
            .contains("Hostname:"),
        "{peer}"
    );

    // a job never overlaps itself, and the time limit stops it
    let started = trigger(&birdd, &service, "slow").await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    let again = trigger(&birdd, &service, "slow").await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{:?}", again.body);
    let slow = finished(&birdd, &service, "slow", &started.body).await;
    assert_eq!(slow["status"], "timed_out", "{slow}");
    assert!(slow["exit_code"].is_null(), "{slow}");
    let took = slow["finished_at"].as_i64().unwrap() - slow["started_at"].as_i64().unwrap();
    assert!(took < 30, "the limit did not stop it: {slow}");
    let after = trigger(&birdd, &service, "slow").await;
    assert_eq!(after.status, StatusCode::ACCEPTED, "{:?}", after.body);
    finished(&birdd, &service, "slow", &after.body).await;

    let history = runs(&birdd, &service, "slow").await;
    assert_eq!(history.len(), 2, "{history:?}");
    assert_eq!(history[0]["id"], after.body["id"], "newest first");
    let listed = birdd.get(&root, &format!("{service}/cron")).await.body;
    assert_eq!(listed[0]["last_run"]["status"], "failed", "{listed}");

    let missing = trigger(&birdd, &service, "nope").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND, "{:?}", missing.body);
    let missing_run = birdd
        .get(
            &root,
            &format!(
                "{service}/cron/hello/runs/{}",
                fails["id"].as_str().unwrap()
            ),
        )
        .await;
    assert_eq!(
        missing_run.status,
        StatusCode::NOT_FOUND,
        "{:?}",
        missing_run.body
    );

    birdd.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn only_successful_deploys_with_a_job_list_change_jobs() {
    let birdd = Birdd::start().await;
    let production = scope("default", "production");
    let service = format!("{production}/services/web");
    let duplicate = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [
            { "name": "a", "schedule": "@daily", "command": ["true"] },
            { "name": "a", "schedule": "@hourly", "command": ["true"] }
        ] }),
    )
    .await;
    assert_eq!(
        duplicate.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        duplicate.body
    );
    let bad_schedule = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [{ "name": "a", "schedule": "61 * * * *", "command": ["true"] }] }),
    )
    .await;
    assert_eq!(
        bad_schedule.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{:?}",
        bad_schedule.body
    );

    let first = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [
            { "name": "hello", "schedule": "@daily", "command": ["sh", "-c", "echo hi"] },
            { "name": "other", "schedule": "@hourly", "command": ["true"] }
        ] }),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.body);
    let started = trigger(&birdd, &service, "hello").await;
    finished(&birdd, &service, "hello", &started.body).await;

    // a deploy without jobs keeps them, a failed one changes nothing, an empty list removes them
    let plain = deploy_with_jobs(&birdd, &production, json!({})).await;
    assert_eq!(plain.status, StatusCode::OK, "{:?}", plain.body);
    assert_eq!(job_names(&birdd, &service).await.len(), 2);
    let broken = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "command": ["false"], "cron": [{ "name": "third", "schedule": "@daily", "command": ["true"] }] }),
    )
    .await;
    assert_eq!(broken.status, StatusCode::BAD_GATEWAY, "{:?}", broken.body);
    assert_eq!(job_names(&birdd, &service).await.len(), 2);
    let kept = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [{ "name": "hello", "schedule": "@daily", "command": ["true"] }] }),
    )
    .await;
    assert_eq!(kept.status, StatusCode::OK, "{:?}", kept.body);
    assert_eq!(job_names(&birdd, &service).await, ["hello"]);
    assert_eq!(
        runs(&birdd, &service, "hello").await.len(),
        1,
        "a kept job keeps its runs"
    );
    let cleared = deploy_with_jobs(&birdd, &production, json!({ "cron": [] })).await;
    assert_eq!(cleared.status, StatusCode::OK, "{:?}", cleared.body);
    assert_eq!(job_names(&birdd, &service).await, Vec::<String>::new());
    birdd.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn schedules_run_skip_stopped_services_and_restarts_interrupt_runs() {
    let mut birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let production = scope("default", "production");
    let service = format!("{production}/services/web");
    let deployed = deploy_with_jobs(
        &birdd,
        &production,
        json!({ "cron": [
            { "name": "tick", "schedule": "* * * * *", "command": ["sh", "-c", "echo tick"] },
            { "name": "long", "schedule": "0 0 1 1 *", "command": ["sleep", "300"] }
        ] }),
    )
    .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);

    // within a minute and the scheduler's tick
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let tick = loop {
        let found = runs(&birdd, &service, "tick").await;
        if let Some(run) = found.iter().find(|run| run["status"] == "succeeded") {
            break run.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "tick never ran on its own: {found:?}"
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    assert_eq!(tick["trigger"], "schedule", "{tick}");
    let started = tick["started_at"].as_i64().unwrap();
    assert!(
        started % 60 < 20,
        "it ran well after the minute began: {tick}"
    );
    let output = birdd
        .get(
            &root,
            &format!("{service}/cron/tick/runs/{}", tick["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(output.body["output"], "tick\n");

    // a run in progress when birdd stops is recorded as interrupted, and starts again on request
    let long = trigger(&birdd, &service, "long").await;
    assert_eq!(long.status, StatusCode::ACCEPTED, "{:?}", long.body);
    tokio::time::sleep(Duration::from_secs(2)).await;
    birdd.restart().await;
    let path = format!(
        "{service}/cron/long/runs/{}",
        long.body["id"].as_str().unwrap()
    );
    let interrupted = birdd.get(&root, &path).await.body;
    assert_eq!(interrupted["status"], "interrupted", "{interrupted}");
    let again = trigger(&birdd, &service, "long").await;
    assert_eq!(again.status, StatusCode::ACCEPTED, "{:?}", again.body);

    // a stopped service's times pass as skipped runs that say why
    let stopped = birdd
        .post(&root, &format!("{service}/stop"), json!(null))
        .await;
    assert!(stopped.status.is_success(), "{:?}", stopped.body);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let skipped = loop {
        let found = runs(&birdd, &service, "tick").await;
        if let Some(run) = found.iter().find(|run| run["status"] == "skipped") {
            break run.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no skipped run: {found:?}"
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    let detail = birdd
        .get(
            &root,
            &format!(
                "{service}/cron/tick/runs/{}",
                skipped["id"].as_str().unwrap()
            ),
        )
        .await
        .body;
    assert!(
        detail["output"]
            .as_str()
            .unwrap_or_default()
            .contains("stopped"),
        "{detail}"
    );
    let ticks = runs(&birdd, &service, "tick").await;
    assert!(
        ticks.iter().all(|run| run["trigger"] == "schedule"),
        "{ticks:?}"
    );
    birdd.stop().await;
}
