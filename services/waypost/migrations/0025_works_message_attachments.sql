-- Message-only attachment reservations; never expose the independent memory/blob API.
CREATE TABLE works_message_attachments (
 attachment_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, sender_assistant_id TEXT NOT NULL,
 request_id TEXT NOT NULL, fingerprint TEXT NOT NULL, object_key TEXT NOT NULL,
 content_type TEXT NOT NULL, filename_ciphertext TEXT, size_bytes INTEGER NOT NULL, sha256 TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','uploaded','linked','expired')),
 message_id TEXT, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, cleanup_checked_at INTEGER,
 UNIQUE(account_id,sender_assistant_id,request_id)
);
CREATE INDEX works_message_attachments_expiry ON works_message_attachments(state,expires_at);
CREATE INDEX works_message_attachments_message ON works_message_attachments(account_id,message_id,state);
