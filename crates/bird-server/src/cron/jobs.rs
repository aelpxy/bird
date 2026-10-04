use std::collections::BTreeSet;

use bird_api::{CronJobSummary, CronRunSummary};
use bird_core::{CronJob, CronJobSpec, CronRun, Name, ServiceId};

use crate::login::unix_now;
use crate::state::AppState;
use crate::{Error, Result};

pub(crate) fn check_specs(specs: &[CronJobSpec]) -> Result<()> {
    let mut seen = BTreeSet::new();
    match specs.iter().find(|spec| !seen.insert(&spec.name)) {
        Some(duplicate) => Err(Error::DuplicateCronJob(duplicate.name.clone())),
        None => Ok(()),
    }
}

pub(crate) async fn set_jobs(
    state: &AppState,
    service_id: ServiceId,
    specs: Vec<CronJobSpec>,
) -> Result<Vec<CronJob>> {
    state
        .db
        .call(move |store| store.set_cron_jobs(service_id, &specs))
        .await
}

pub(crate) async fn find_job(
    state: &AppState,
    service_id: ServiceId,
    service: &Name,
    job: &Name,
) -> Result<CronJob> {
    let jobs = state
        .db
        .call(move |store| store.list_cron_jobs(service_id))
        .await?;
    jobs.into_iter()
        .find(|found| found.name == *job)
        .ok_or_else(|| Error::CronJobNotFound {
            service: service.clone(),
            job: job.clone(),
        })
}

pub(crate) async fn summaries(
    state: &AppState,
    service_id: ServiceId,
) -> Result<Vec<CronJobSummary>> {
    let jobs = state
        .db
        .call(move |store| {
            store
                .list_cron_jobs(service_id)?
                .into_iter()
                .map(|job| {
                    let last = store.list_cron_runs(job.id, 1)?.into_iter().next();
                    Ok((job, last))
                })
                .collect::<bird_store::Result<Vec<_>>>()
        })
        .await?;
    let now = unix_now();
    Ok(jobs
        .into_iter()
        .map(|(job, last)| CronJobSummary {
            next_run_at: job.schedule.next_after(now),
            last_run: last.map(|run| summary(&run)),
            name: job.name,
            schedule: job.schedule,
            command: job.command,
            timeout: job.timeout,
        })
        .collect())
}

#[must_use]
pub(crate) fn summary(run: &CronRun) -> CronRunSummary {
    CronRunSummary {
        id: run.id,
        trigger: run.trigger,
        status: run.status,
        exit_code: run.exit_code,
        started_at: run.started_at,
        finished_at: run.finished_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str) -> CronJobSpec {
        CronJobSpec {
            name: name.parse().unwrap(),
            schedule: "* * * * *".parse().unwrap(),
            command: serde_json::from_str(r#"["true"]"#).unwrap(),
            timeout: None,
        }
    }

    #[test]
    fn refuses_a_name_listed_twice() {
        assert!(check_specs(&[spec("a"), spec("b")]).is_ok());
        assert!(matches!(
            check_specs(&[spec("a"), spec("b"), spec("a")]),
            Err(Error::DuplicateCronJob(name)) if name.as_str() == "a"
        ));
    }
}
