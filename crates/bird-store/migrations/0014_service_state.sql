ALTER TABLE services ADD COLUMN state TEXT NOT NULL DEFAULT 'running' CHECK (state IN ('running', 'stopped'));
