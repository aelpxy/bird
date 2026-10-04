use bird_core::{
    Command, CronJob, CronJobId, CronJobSpec, CronRun, CronRunId, CronRunStatus, CronTimeout,
    CronTrigger, ServiceId,
};
use rusqlite::{Row, params};

use crate::error::{Error, expect_changed, write_error};
use crate::rows::{command, parse};
use crate::store::now;
use crate::{Result, Store};

const JOB_COLUMNS: &str =
    "id, service_id, name, schedule, command, timeout_secs, last_due_at, created_at";
const RUN_COLUMNS: &str = "id, job_id, trigger, status, exit_code, started_at, finished_at, output";

impl Store {
    // makes the service's jobs exactly `specs`; a kept name keeps its id, history and the times it
    // already handled
    pub fn set_cron_jobs(
        &mut self,
        service_id: ServiceId,
        specs: &[CronJobSpec],
    ) -> Result<Vec<CronJob>> {
        self.transaction(|store| {
            let names: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
            for job in store.list_cron_jobs(service_id)? {
                if !names.contains(&job.name.as_str()) {
                    store.execute("DELETE FROM cron_jobs WHERE id = ?1", [job.id.to_string()])?;
                }
            }
            for spec in specs {
                let timeout = spec.timeout.unwrap_or_default();
                store
                    .execute(
                        "INSERT INTO cron_jobs (id, service_id, name, schedule, command, timeout_secs, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                         ON CONFLICT (service_id, name) DO UPDATE SET
                             schedule = excluded.schedule, command = excluded.command,
                             timeout_secs = excluded.timeout_secs",
                        params![
                            CronJobId::generate().to_string(),
                            service_id.to_string(),
                            spec.name.as_str(),
                            spec.schedule.as_str(),
                            encode(&spec.command)?,
                            timeout.secs(),
                            now()
                        ],
                    )
                    .map_err(write_error("cron job"))?;
            }
            store.list_cron_jobs(service_id)
        })
    }

    pub fn list_cron_jobs(&self, service_id: ServiceId) -> Result<Vec<CronJob>> {
        self.query_all(
            &format!("SELECT {JOB_COLUMNS} FROM cron_jobs WHERE service_id = ?1 ORDER BY name"),
            [service_id.to_string()],
            job,
        )
    }

    pub fn all_cron_jobs(&self) -> Result<Vec<CronJob>> {
        self.query_all(
            &format!("SELECT {JOB_COLUMNS} FROM cron_jobs ORDER BY id"),
            [],
            job,
        )
    }

    // true for the first caller only, so a scheduled time starts one run however often it is asked
    pub fn claim_cron_due(&self, job_id: CronJobId, due_at: i64) -> Result<bool> {
        let changed = self.execute(
            "UPDATE cron_jobs SET last_due_at = ?2
             WHERE id = ?1 AND (last_due_at IS NULL OR last_due_at < ?2)",
            params![job_id.to_string(), due_at],
        )?;
        Ok(changed == 1)
    }

    pub fn start_cron_run(
        &self,
        job_id: CronJobId,
        trigger: CronTrigger,
        status: CronRunStatus,
        output: &str,
    ) -> Result<CronRun> {
        let at = now();
        let finished_at = (status != CronRunStatus::Running).then_some(at);
        let run = CronRun {
            id: CronRunId::generate(),
            job_id,
            trigger,
            status,
            exit_code: None,
            started_at: at,
            finished_at,
            output: output.to_owned(),
        };
        self.execute(
            "INSERT INTO cron_runs (id, job_id, trigger, status, started_at, finished_at, output)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run.id.to_string(),
                job_id.to_string(),
                trigger.as_str(),
                status.as_str(),
                at,
                finished_at,
                output
            ],
        )
        .map_err(write_error("cron run"))?;
        Ok(run)
    }

    pub fn finish_cron_run(
        &self,
        id: CronRunId,
        status: CronRunStatus,
        exit_code: Option<i32>,
        output: &str,
    ) -> Result<()> {
        let changed = self.execute(
            "UPDATE cron_runs SET status = ?2, exit_code = ?3, finished_at = ?4, output = ?5
             WHERE id = ?1",
            params![id.to_string(), status.as_str(), exit_code, now(), output],
        )?;
        expect_changed(changed, "cron run")
    }

    // newest first
    pub fn list_cron_runs(&self, job_id: CronJobId, limit: u32) -> Result<Vec<CronRun>> {
        self.query_all(
            &format!(
                "SELECT {RUN_COLUMNS} FROM cron_runs WHERE job_id = ?1
                 ORDER BY started_at DESC, rowid DESC LIMIT ?2"
            ),
            params![job_id.to_string(), limit],
            run,
        )
    }

    pub fn cron_run(&self, job_id: CronJobId, id: CronRunId) -> Result<Option<CronRun>> {
        self.query_one(
            &format!("SELECT {RUN_COLUMNS} FROM cron_runs WHERE job_id = ?1 AND id = ?2"),
            params![job_id.to_string(), id.to_string()],
            run,
        )
    }

    // keeps the newest `keep` runs of a job
    pub fn prune_cron_runs(&self, job_id: CronJobId, keep: u32) -> Result<usize> {
        Ok(self.execute(
            "DELETE FROM cron_runs WHERE job_id = ?1 AND id NOT IN (
                 SELECT id FROM cron_runs WHERE job_id = ?1
                 ORDER BY started_at DESC, rowid DESC LIMIT ?2
             )",
            params![job_id.to_string(), keep],
        )?)
    }

    // runs birdd was in the middle of when it stopped
    pub fn interrupt_cron_runs(&self) -> Result<usize> {
        Ok(self.execute(
            "UPDATE cron_runs SET status = 'interrupted', finished_at = ?1
             WHERE status = 'running'",
            [now()],
        )?)
    }
}

fn encode(command: &Command) -> Result<String> {
    serde_json::to_string(command.args()).map_err(Error::Encode)
}

fn job(row: &Row<'_>) -> rusqlite::Result<CronJob> {
    let timeout: u32 = row.get(5)?;
    Ok(CronJob {
        id: parse(row, 0)?,
        service_id: parse(row, 1)?,
        name: parse(row, 2)?,
        schedule: parse(row, 3)?,
        command: command(row, 4)?.ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(4, "command".to_owned(), rusqlite::types::Type::Null)
        })?,
        timeout: CronTimeout::try_from(timeout).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Integer,
                Box::new(err),
            )
        })?,
        last_due_at: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn run(row: &Row<'_>) -> rusqlite::Result<CronRun> {
    Ok(CronRun {
        id: parse(row, 0)?,
        job_id: parse(row, 1)?,
        trigger: parse(row, 2)?,
        status: parse(row, 3)?,
        exit_code: row.get(4)?,
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        output: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use bird_core::{CronJobSpec, CronRunStatus, CronTrigger};

    use super::*;
    use crate::testing::{name, setup};

    fn spec(job: &str, schedule: &str, command: &[&str]) -> CronJobSpec {
        CronJobSpec {
            name: name(job),
            schedule: schedule.parse().unwrap(),
            command: Command::try_from(
                command
                    .iter()
                    .map(|arg| (*arg).to_owned())
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            timeout: None,
        }
    }

    #[test]
    fn replaces_jobs_and_keeps_what_stays() {
        let (mut store, service) = setup();
        let first = store
            .set_cron_jobs(
                service.id,
                &[
                    spec("cleanup", "0 3 * * *", &["clean"]),
                    spec("mail", "@hourly", &["send"]),
                ],
            )
            .unwrap();
        assert_eq!(first.len(), 2);
        let cleanup = first
            .iter()
            .find(|job| job.name == name("cleanup"))
            .unwrap()
            .clone();
        assert_eq!(cleanup.timeout, CronTimeout::DEFAULT);
        assert!(store.claim_cron_due(cleanup.id, 100).unwrap());

        let changed = CronJobSpec {
            timeout: Some("5m".parse().unwrap()),
            ..spec("cleanup", "30 4 * * *", &["clean", "--all"])
        };
        let second = store.set_cron_jobs(service.id, &[changed]).unwrap();
        assert_eq!(second.len(), 1);
        let kept = &second[0];
        assert_eq!((kept.id, kept.last_due_at), (cleanup.id, Some(100)));
        assert_eq!(kept.schedule.as_str(), "30 4 * * *");
        assert_eq!(kept.command.args(), ["clean", "--all"]);
        assert_eq!(kept.timeout.secs(), 300);
        assert_eq!(store.set_cron_jobs(service.id, &[]).unwrap(), Vec::new());
    }

    #[test]
    fn a_scheduled_time_is_claimed_once() {
        let (mut store, service) = setup();
        let job = store
            .set_cron_jobs(service.id, &[spec("tick", "* * * * *", &["true"])])
            .unwrap()
            .remove(0);
        assert!(store.claim_cron_due(job.id, 60).unwrap());
        assert!(!store.claim_cron_due(job.id, 60).unwrap());
        assert!(!store.claim_cron_due(job.id, 0).unwrap());
        assert!(store.claim_cron_due(job.id, 120).unwrap());
    }

    #[test]
    fn records_prunes_and_interrupts_runs() {
        let (mut store, service) = setup();
        let job = store
            .set_cron_jobs(service.id, &[spec("tick", "* * * * *", &["true"])])
            .unwrap()
            .remove(0);
        let skipped = store
            .start_cron_run(
                job.id,
                CronTrigger::Schedule,
                CronRunStatus::Skipped,
                "still running",
            )
            .unwrap();
        assert!(skipped.finished_at.is_some());
        let done = store
            .start_cron_run(job.id, CronTrigger::Manual, CronRunStatus::Running, "")
            .unwrap();
        store
            .finish_cron_run(done.id, CronRunStatus::Failed, Some(3), "oops\n")
            .unwrap();
        let found = store.cron_run(job.id, done.id).unwrap().unwrap();
        assert_eq!(
            (found.status, found.exit_code, found.output.as_str()),
            (CronRunStatus::Failed, Some(3), "oops\n")
        );
        let running = store
            .start_cron_run(job.id, CronTrigger::Schedule, CronRunStatus::Running, "")
            .unwrap();

        let newest: Vec<_> = store
            .list_cron_runs(job.id, 10)
            .unwrap()
            .into_iter()
            .map(|run| run.id)
            .collect();
        assert_eq!(newest, [running.id, done.id, skipped.id]);
        assert_eq!(store.interrupt_cron_runs().unwrap(), 1);
        assert_eq!(
            store.cron_run(job.id, running.id).unwrap().unwrap().status,
            CronRunStatus::Interrupted
        );
        assert_eq!(store.prune_cron_runs(job.id, 1).unwrap(), 2);
        assert_eq!(store.list_cron_runs(job.id, 10).unwrap().len(), 1);
    }
}
