-- Additive control-plane grants. OAuth families remain the parent connection;
-- grants are independent and can only be reduced after consent.
CREATE TABLE unified_connection_grants (
 request_id TEXT PRIMARY KEY REFERENCES oauth_requests(request_id),
 account_id TEXT NOT NULL REFERENCES accounts(account_id),
 messages_enabled INTEGER NOT NULL DEFAULT 1 CHECK(messages_enabled IN(0,1)),
 keys_enabled INTEGER NOT NULL DEFAULT 0 CHECK(keys_enabled IN(0,1)),
 key_version INTEGER NOT NULL DEFAULT 1 CHECK(key_version>0),
 created_at INTEGER NOT NULL
);
CREATE INDEX unified_connection_grants_owner ON unified_connection_grants(account_id);
