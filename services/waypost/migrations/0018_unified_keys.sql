-- Applied to the separate keys database too. These are local business grants,
-- not public OAuth credentials. The control-plane family is checked per call.
CREATE TABLE unified_key_grants (
 request_id TEXT PRIMARY KEY,
 account_id TEXT NOT NULL REFERENCES accounts(account_id),
 scope TEXT NOT NULL,
 consent_digest TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 can_create INTEGER NOT NULL CHECK(can_create IN(0,1)),
 all_read INTEGER NOT NULL CHECK(all_read IN(0,1)),
 all_write INTEGER NOT NULL CHECK(all_write IN(0,1)),
 revoked_at INTEGER,
 created_at INTEGER NOT NULL
);
CREATE TABLE unified_key_consent_entries (
 request_id TEXT NOT NULL REFERENCES unified_key_grants(request_id),
 entry_id TEXT NOT NULL REFERENCES key_entries(entry_id),
 can_read INTEGER NOT NULL CHECK(can_read IN(0,1)),
 can_write INTEGER NOT NULL CHECK(can_write IN(0,1)),
 PRIMARY KEY(request_id,entry_id)
);
