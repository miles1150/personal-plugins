-- A browser-approved mount is bound to the individual OAuth request and to
-- the exact assistant policy the account owner reviewed.
CREATE TABLE works_oauth_mount_intents (
 request_id TEXT PRIMARY KEY REFERENCES oauth_requests(request_id),
 account_id TEXT NOT NULL REFERENCES accounts(account_id),
 assistant_id TEXT NOT NULL REFERENCES works_assistants(assistant_id),
 policy_revision INTEGER NOT NULL,
 created_at INTEGER NOT NULL
);
