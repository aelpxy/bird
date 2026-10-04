ALTER TABLE services ADD COLUMN health_path TEXT;
ALTER TABLE services ADD COLUMN health_timeout_secs INTEGER NOT NULL DEFAULT 60 CHECK (health_timeout_secs BETWEEN 1 AND 3600);
