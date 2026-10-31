-- Account-owned encrypted snapshots, independent of legacy Actor/Space records.
CREATE TABLE works_material_revisions (
 account_id TEXT NOT NULL REFERENCES accounts(account_id), collection_id TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision > 0), assistant_id TEXT NOT NULL,
 request_id TEXT NOT NULL, fingerprint TEXT NOT NULL, ciphertext TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 PRIMARY KEY(account_id,collection_id,revision),
 UNIQUE(account_id,assistant_id,request_id)
);
CREATE INDEX works_material_account ON works_material_revisions(account_id,collection_id,revision DESC);
