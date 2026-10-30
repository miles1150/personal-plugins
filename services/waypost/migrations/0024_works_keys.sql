-- The cache is an additional local write guard, never the source of authority.
-- Revisions only advance; control DB authorization is checked on each request.
CREATE TABLE works_key_policy_cache (
 assistant_id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(account_id),
 revision INTEGER NOT NULL, keys_read INTEGER NOT NULL, keys_write INTEGER NOT NULL,
 keys_create INTEGER NOT NULL, key_scope TEXT NOT NULL
);
