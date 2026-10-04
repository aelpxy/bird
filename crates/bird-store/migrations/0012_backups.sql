CREATE TABLE backups (
    id TEXT PRIMARY KEY,
    environment_id TEXT NOT NULL REFERENCES environments (id) ON DELETE CASCADE,
    service_name TEXT NOT NULL,
    trigger TEXT NOT NULL,
    storage TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX backups_by_service ON backups (environment_id, service_name);

CREATE TABLE backup_volumes (
    backup_id TEXT NOT NULL REFERENCES backups (id) ON DELETE CASCADE,
    volume_name TEXT NOT NULL,
    lineage TEXT,
    key TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    PRIMARY KEY (backup_id, volume_name)
) STRICT;
