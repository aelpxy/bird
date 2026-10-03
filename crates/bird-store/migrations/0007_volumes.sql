CREATE TABLE volumes (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    mount_path TEXT NOT NULL,
    lineage TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE (service_id, name),
    UNIQUE (service_id, mount_path)
) STRICT;
