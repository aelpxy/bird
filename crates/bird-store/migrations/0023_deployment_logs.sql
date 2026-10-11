CREATE TABLE deployment_logs (
    deployment_id TEXT PRIMARY KEY REFERENCES deployments (id) ON DELETE CASCADE,
    lines TEXT NOT NULL
);
