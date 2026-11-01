-- Read-only shared snapshots of design artifacts. Snapshot bytes live in R2;
-- links stay off until the account owner turns them on.
CREATE TABLE works_shares (
 share_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id),
 assistant_id TEXT NOT NULL, title TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('canvas','slides','page')),
 current_version INTEGER NOT NULL DEFAULT 0 CHECK(current_version >= 0),
 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX works_shares_account ON works_shares(account_id,updated_at DESC);
CREATE TABLE works_share_versions (
 share_id TEXT NOT NULL REFERENCES works_shares(share_id), version INTEGER NOT NULL CHECK(version > 0),
 object_key TEXT NOT NULL UNIQUE, size_bytes INTEGER NOT NULL CHECK(size_bytes > 0),
 sha256 BLOB NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','ready')),
 upload_digest BLOB UNIQUE, upload_expires_at INTEGER NOT NULL,
 created_at INTEGER NOT NULL, ready_at INTEGER,
 PRIMARY KEY(share_id,version)
);
CREATE TABLE works_share_links (
 link_id TEXT PRIMARY KEY, share_id TEXT NOT NULL REFERENCES works_shares(share_id),
 role TEXT NOT NULL CHECK(role IN ('comment','view')), token_digest BLOB NOT NULL UNIQUE,
 token_cipher TEXT NOT NULL, days INTEGER NOT NULL CHECK(days BETWEEN 1 AND 30),
 status TEXT NOT NULL CHECK(status IN ('pending','active','revoked')),
 created_at INTEGER NOT NULL, activated_at INTEGER, expires_at INTEGER, revoked_at INTEGER
);
CREATE INDEX works_share_links_share ON works_share_links(share_id,created_at);
CREATE TABLE works_share_comments (
 comment_id TEXT PRIMARY KEY, share_id TEXT NOT NULL REFERENCES works_shares(share_id),
 version INTEGER NOT NULL, parent_id TEXT, link_id TEXT,
 author_kind TEXT NOT NULL CHECK(author_kind IN ('guest','agent')), author_name TEXT NOT NULL,
 body TEXT NOT NULL, anchor TEXT, created_at INTEGER NOT NULL
);
CREATE INDEX works_share_comments_share ON works_share_comments(share_id,created_at);
