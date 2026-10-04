use bird_core::{CronRunId, Name};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum CronCommand {
    /// List the service's cron jobs with their next and last run; set them with [[cron]] in bird.toml
    #[command(visible_alias = "ls")]
    List,
    /// List a job's latest runs, newest first
    Runs { job: Name },
    /// Run a job now, print its output and exit with its code
    Run { job: Name },
    /// Print the output of a job's run, the latest by default
    Output { job: Name, run: Option<CronRunId> },
}
