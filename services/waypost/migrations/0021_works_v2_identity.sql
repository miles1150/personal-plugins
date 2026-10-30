-- Additive v2 identity. Bootstrap credentials do not impersonate a legacy actor.
CREATE TABLE works_families (
 family_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id),
 client_id TEXT NOT NULL, client_revision INTEGER, resource TEXT NOT NULL, installation_id TEXT NOT NULL,
 created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, revoked_at INTEGER
);
CREATE TABLE works_access_tokens (
 token_digest BLOB PRIMARY KEY CHECK(length(token_digest)=32), family_id TEXT NOT NULL REFERENCES works_families(family_id), expires_at INTEGER NOT NULL
);
CREATE INDEX works_access_family ON works_access_tokens(family_id);
CREATE TABLE works_refresh_tokens (
 token_digest BLOB PRIMARY KEY CHECK(length(token_digest)=32), family_id TEXT NOT NULL REFERENCES works_families(family_id), expires_at INTEGER NOT NULL, used_at INTEGER, redeem_nonce TEXT UNIQUE
);
CREATE INDEX works_refresh_family ON works_refresh_tokens(family_id);
CREATE INDEX works_families_client ON works_families(client_id);
CREATE TABLE works_installations (
 installation_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id),
 label TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','disabled')), created_at INTEGER NOT NULL
);
CREATE TABLE works_assistants (
 assistant_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id), name TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','paused')), policy_revision INTEGER NOT NULL DEFAULT 1,
 messages_enabled INTEGER NOT NULL DEFAULT 0 CHECK(messages_enabled IN (0,1)), keys_read INTEGER NOT NULL DEFAULT 0 CHECK(keys_read IN (0,1)),
 keys_write INTEGER NOT NULL DEFAULT 0 CHECK(keys_write IN (0,1)), keys_create INTEGER NOT NULL DEFAULT 0 CHECK(keys_create IN (0,1)),
 key_scope TEXT NOT NULL DEFAULT 'all' CHECK(key_scope IN ('all','selected')), default_session_id TEXT, route_revision INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL
);
CREATE INDEX works_assistants_owner ON works_assistants(account_id);
CREATE TABLE works_attachments (
 attachment_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id), assistant_id TEXT NOT NULL REFERENCES works_assistants(assistant_id), installation_id TEXT NOT NULL REFERENCES works_installations(installation_id), status TEXT NOT NULL CHECK(status IN ('active','revoked')), created_at INTEGER NOT NULL, revoked_at INTEGER
);
CREATE UNIQUE INDEX works_attachment_active ON works_attachments(assistant_id,installation_id) WHERE status='active';
CREATE TABLE works_mount_requests (
 request_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id), installation_id TEXT NOT NULL REFERENCES works_installations(installation_id), family_id TEXT NOT NULL REFERENCES works_families(family_id), assistant_id TEXT NOT NULL REFERENCES works_assistants(assistant_id), policy_revision INTEGER NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','approved','denied')), created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, replaced_by TEXT REFERENCES works_mount_requests(request_id)
);
CREATE INDEX works_mount_requests_owner ON works_mount_requests(account_id,status,expires_at);
