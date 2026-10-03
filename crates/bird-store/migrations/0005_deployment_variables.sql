CREATE TABLE deployment_variables (
    deployment_id TEXT NOT NULL REFERENCES deployments (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (deployment_id, key)
) STRICT;

INSERT INTO deployment_variables (deployment_id, key, value)
SELECT d.id, v.key, v.value FROM deployments d JOIN variables v ON v.service_id = d.service_id;

CREATE TRIGGER snapshot_deployment_variables AFTER INSERT ON deployments
BEGIN
    INSERT INTO deployment_variables (deployment_id, key, value)
    SELECT NEW.id, key, value FROM variables WHERE service_id = NEW.service_id;
END;
