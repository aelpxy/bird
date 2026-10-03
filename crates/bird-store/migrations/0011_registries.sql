CREATE TABLE registries (
    host TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    insecure INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
) STRICT;
