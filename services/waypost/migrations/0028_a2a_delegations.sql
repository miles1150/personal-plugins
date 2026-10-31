-- Explicit owner-approved authority, separate from messages and claims.
CREATE TABLE works_a2a_delegations (
 account_id TEXT NOT NULL, attachment_id TEXT NOT NULL, sender_assistant_id TEXT NOT NULL,
 actions INTEGER NOT NULL CHECK(actions >= 0 AND actions <= 31),
 revision INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
 PRIMARY KEY(account_id, attachment_id, sender_assistant_id)
);
