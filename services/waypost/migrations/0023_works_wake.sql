-- Bridge credentials and receiver credentials remain independent.
CREATE TABLE works_wake_bindings (
 binding_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, assistant_id TEXT NOT NULL,
 bridge_installation_id TEXT NOT NULL, bridge_family_id TEXT NOT NULL, bridge_attachment_id TEXT NOT NULL,
 execution_attachment_id TEXT NOT NULL,
 session_id TEXT NOT NULL, execution_installation_id TEXT NOT NULL,
 registration_revision INTEGER NOT NULL, request_id TEXT NOT NULL, target TEXT NOT NULL,
 created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
 confirmed_at INTEGER, receiver_family_id TEXT, last_poll_at INTEGER,
 UNIQUE(bridge_family_id,request_id)
);
CREATE INDEX works_wake_session ON works_wake_bindings(account_id,assistant_id,session_id,expires_at);
