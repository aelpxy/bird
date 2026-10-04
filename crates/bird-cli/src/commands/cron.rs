use std::io::Write;
use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{CronJobSummary, CronRunDetail, CronRunSummary};
use bird_core::{CronRunId, CronRunStatus, Name};

use super::RemoteExit;
use super::history::{ago, unix_now};
use super::table::render;
use crate::args::CronCommand;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, Spinner};

const TIMEOUT: Duration = Duration::from_secs(30);
const POLL_EVERY: Duration = Duration::from_secs(1);

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: CronCommand,
    out: Output,
) -> Result<()> {
    match command {
        CronCommand::List => list(client, name, out).await,
        CronCommand::Runs { job } => runs(client, name, &job, out).await,
        CronCommand::Run { job } => run_now(client, name, &job, out).await,
        CronCommand::Output { job, run } => output(client, name, &job, run, out).await,
    }
}

async fn list(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let jobs: Vec<CronJobSummary> = client
        .get(&client.scoped(&format!("services/{name}/cron")), TIMEOUT)
        .await?;
    if out.json(&jobs)? {
        return Ok(());
    }
    if jobs.is_empty() {
        println!("{name} has no cron jobs, add them with [[cron]] in bird.toml");
        return Ok(());
    }
    let now = unix_now();
    let rows: Vec<Vec<String>> = jobs
        .iter()
        .map(|job| {
            vec![
                job.name.to_string(),
                job.schedule.to_string(),
                job.command.args().join(" "),
                job.next_run_at
                    .map_or_else(|| "never".to_owned(), |at| within(at - now)),
                job.last_run.as_ref().map_or_else(
                    || "-".to_owned(),
                    |run| format!("{} {}", run.status, ago(now - run.started_at)),
                ),
            ]
        })
        .collect();
    print!(
        "{}",
        render(&["JOB", "SCHEDULE (UTC)", "COMMAND", "NEXT", "LAST"], &rows)
    );
    Ok(())
}

async fn runs(client: &ApiClient, name: &Name, job: &Name, out: Output) -> Result<()> {
    let runs: Vec<CronRunSummary> = client
        .get(
            &client.scoped(&format!("services/{name}/cron/{job}/runs")),
            TIMEOUT,
        )
        .await?;
    if out.json(&runs)? {
        return Ok(());
    }
    if runs.is_empty() {
        println!(
            "{job} has not run yet, run it now with {}",
            style::out(Paint::Bold, format!("bird cron run {job}"))
        );
        return Ok(());
    }
    let now = unix_now();
    let rows: Vec<Vec<String>> = runs
        .iter()
        .map(|run| {
            vec![
                run.id.to_string(),
                run.trigger.to_string(),
                run.status.to_string(),
                run.exit_code
                    .map_or_else(|| "-".to_owned(), |c| c.to_string()),
                ago(now - run.started_at),
                run.finished_at
                    .map_or_else(|| "-".to_owned(), |at| format!("{}s", at - run.started_at)),
            ]
        })
        .collect();
    print!(
        "{}",
        render(
            &["RUN", "TRIGGER", "STATUS", "EXIT", "STARTED", "TOOK"],
            &rows
        )
    );
    Ok(())
}

// waits for the run like `bird run` would, so scripts can use the exit code
async fn run_now(client: &ApiClient, name: &Name, job: &Name, out: Output) -> Result<()> {
    let started: CronRunSummary = client
        .post(
            &client.scoped(&format!("services/{name}/cron/{job}/runs")),
            &(),
            TIMEOUT,
        )
        .await?;
    let spinner = Spinner::start(format!("running {job} on {name}"));
    let path = client.scoped(&format!("services/{name}/cron/{job}/runs/{}", started.id));
    let finished = loop {
        let run: CronRunDetail = client.get(&path, TIMEOUT).await?;
        if run.run.status != CronRunStatus::Running {
            break run;
        }
        tokio::time::sleep(POLL_EVERY).await;
    };
    drop(spinner);
    if out.json(&finished)? {
        return Ok(());
    }
    print_output(&finished)?;
    match (finished.run.status, finished.run.exit_code) {
        (CronRunStatus::Succeeded, _) => Ok(()),
        (_, Some(code)) => Err(RemoteExit(code).into()),
        (status, None) => bail!("{job} {status}"),
    }
}

async fn output(
    client: &ApiClient,
    name: &Name,
    job: &Name,
    run: Option<CronRunId>,
    out: Output,
) -> Result<()> {
    let id = if let Some(id) = run {
        id
    } else {
        let runs: Vec<CronRunSummary> = client
            .get(
                &client.scoped(&format!("services/{name}/cron/{job}/runs")),
                TIMEOUT,
            )
            .await?;
        let Some(latest) = runs.first() else {
            bail!("{job} has not run yet");
        };
        latest.id
    };
    let run: CronRunDetail = client
        .get(
            &client.scoped(&format!("services/{name}/cron/{job}/runs/{id}")),
            TIMEOUT,
        )
        .await?;
    if !out.json(&run)? {
        print_output(&run)?;
    }
    Ok(())
}

fn print_output(run: &CronRunDetail) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(run.output.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn within(seconds: i64) -> String {
    match seconds {
        ..60 => format!("in {}s", seconds.max(0)),
        60..3600 => format!("in {}m", seconds / 60),
        3600..86_400 => format!("in {}h", seconds / 3600),
        _ => format!("in {}d", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_when_the_next_run_is() {
        assert_eq!(within(-3), "in 0s");
        assert_eq!(within(42), "in 42s");
        assert_eq!(within(150), "in 2m");
        assert_eq!(within(7200), "in 2h");
        assert_eq!(within(200_000), "in 2d");
    }
}
