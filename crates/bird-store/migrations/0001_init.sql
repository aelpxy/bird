CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE environments (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (project_id, name)
) STRICT;

CREATE TABLE services (
    id TEXT PRIMARY KEY,
    environment_id TEXT NOT NULL REFERENCES environments (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    image TEXT NOT NULL,
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    created_at INTEGER NOT NULL,
    UNIQUE (environment_id, name)
) STRICT;

CREATE TABLE deployments (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    image TEXT NOT NULL,
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX deployments_by_service ON deployments (service_id, created_at);

CREATE UNIQUE INDEX one_active_deployment ON deployments (service_id) WHERE status = 'active';

CREATE TABLE machines (
    id TEXT PRIMARY KEY,
    deployment_id TEXT NOT NULL REFERENCES deployments (id) ON DELETE CASCADE,
    container_id TEXT,
    state TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX machines_by_deployment ON machines (deployment_id);

CREATE TABLE domains (
    hostname TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX domains_by_service ON domains (service_id);

CREATE TABLE variables (
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (service_id, key)
) STRICT;
