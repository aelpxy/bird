CREATE TABLE cron_jobs (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,
    command TEXT NOT NULL,
    timeout_secs INTEGER NOT NULL CHECK (timeout_secs BETWEEN 1 AND 86400),
    -- the latest scheduled time already handled, so a time fires once
    last_due_at INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (service_id, name)
) STRICT;

CREATE TABLE cron_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES cron_jobs (id) ON DELETE CASCADE,
    trigger TEXT NOT NULL CHECK (trigger IN ('schedule', 'manual')),
    status TEXT NOT NULL
        CHECK (status IN ('running', 'succeeded', 'failed', 'timed_out', 'interrupted', 'skipped')),
    exit_code INTEGER,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    output TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX cron_runs_by_job ON cron_runs (job_id, started_at);
