-- Preserve the old expiry values for audit and recovery. Only currently live,
-- unused refresh credentials of active accounts, installations and registered
-- clients are extended; expired, consumed, revoked or disabled records remain
-- unchanged. This does not restore an already expired credential.
CREATE TABLE works_refresh_lifetime_migration (
 family_id TEXT NOT NULL,
 refresh_digest BLOB NOT NULL,
 old_family_expires_at INTEGER NOT NULL,
 old_refresh_expires_at INTEGER NOT NULL,
 PRIMARY KEY(family_id,refresh_digest)
);
INSERT INTO works_refresh_lifetime_migration(family_id,refresh_digest,old_family_expires_at,old_refresh_expires_at)
 SELECT f.family_id,r.token_digest,f.expires_at,r.expires_at
 FROM works_families f
 JOIN works_refresh_tokens r ON r.family_id=f.family_id
 JOIN accounts a ON a.account_id=f.account_id
 JOIN works_installations i ON i.installation_id=f.installation_id AND i.account_id=f.account_id
 JOIN oauth_dynamic_clients c ON c.client_id=f.client_id AND c.revision=f.client_revision
 WHERE f.revoked_at IS NULL AND f.expires_at>unixepoch('now')*1000+1000
 AND r.used_at IS NULL AND r.expires_at>unixepoch('now')*1000+1000
 AND a.disabled=0 AND i.status='active' AND c.disabled=0 AND c.valid_until>unixepoch('now')*1000+1000;
UPDATE works_refresh_tokens SET expires_at=9007199254740991
 WHERE EXISTS(SELECT 1 FROM works_refresh_lifetime_migration m WHERE m.family_id=works_refresh_tokens.family_id AND m.refresh_digest=works_refresh_tokens.token_digest);
UPDATE works_families SET expires_at=9007199254740991
 WHERE EXISTS(SELECT 1 FROM works_refresh_lifetime_migration m WHERE m.family_id=works_families.family_id);
