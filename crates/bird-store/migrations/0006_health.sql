ALTER TABLE services ADD COLUMN health TEXT NOT NULL DEFAULT 'http' CHECK (health IN ('http', 'tcp'));
