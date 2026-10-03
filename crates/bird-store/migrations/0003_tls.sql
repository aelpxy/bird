CREATE TABLE certificates (
    hostname TEXT PRIMARY KEY,
    chain_pem TEXT NOT NULL,
    key_pem TEXT NOT NULL,
    not_after INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE acme_accounts (
    directory_url TEXT PRIMARY KEY,
    credentials TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;
