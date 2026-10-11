ALTER TABLE services ADD COLUMN proxy TEXT;
ALTER TABLE domains ADD COLUMN redirect_to TEXT REFERENCES domains (hostname) ON DELETE SET NULL;
