CREATE TABLE orgs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE org_members (
    org_id TEXT NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    PRIMARY KEY (org_id, user_id)
) STRICT;

-- birdd puts projects from before orgs into the `default` org at startup, which needs an id
ALTER TABLE projects ADD COLUMN org_id TEXT REFERENCES orgs (id);
CREATE INDEX projects_by_org ON projects (org_id);
