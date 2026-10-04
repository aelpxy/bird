-- what signing in with a password needs; api tokens keep working without any of it
ALTER TABLE users ADD COLUMN password_hash TEXT;
-- base32, set once a code from it was confirmed
ALTER TABLE users ADD COLUMN totp_secret TEXT;
-- shown during setup, not yet confirmed
ALTER TABLE users ADD COLUMN totp_pending TEXT;
-- the newest time step a code was accepted for, so a code works only once
ALTER TABLE users ADD COLUMN totp_last_step INTEGER;

CREATE TABLE recovery_codes (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    hash TEXT NOT NULL,
    PRIMARY KEY (user_id, hash)
) STRICT;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    hash TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    address TEXT,
    agent TEXT
) STRICT;
CREATE INDEX sessions_by_user ON sessions (user_id);
