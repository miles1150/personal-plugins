import sqlite3
import time
import unittest
from pathlib import Path


MIGRATIONS = Path(__file__).resolve().parents[1] / "migrations"
LONG_EXPIRY = 9_007_199_254_740_991


class RefreshLifetimeMigrationTest(unittest.TestCase):
    def test_only_live_credentials_are_extended_and_old_expiries_recorded(self):
        db = sqlite3.connect(":memory:")
        for migration in sorted(MIGRATIONS.glob("*.sql")):
            if migration.name < "0030_works_long_lived_refresh.sql":
                db.executescript(migration.read_text())
        now = int(time.time() * 1000)
        future, past = now + 86_400_000, now - 86_400_000
        cases = ("valid", "family_expired", "refresh_expired", "revoked", "used",
                 "account_disabled", "installation_disabled", "client_disabled", "revision_changed", "client_expired")
        originals = {}
        for i, case in enumerate(cases, 1):
            account, client, installation, family = (f"{prefix}-{case}" for prefix in ("a", "c", "i", "f"))
            digest = bytes([i]) * 32
            db.execute("INSERT INTO accounts(account_id,issuer,subject,disabled,created_at) VALUES(?,?,?,?,?)", (account, "issuer", account, int(case == "account_disabled"), now))
            db.execute("INSERT INTO oauth_dynamic_clients(client_id,source,metadata_json,security_fingerprint,revision,valid_until,disabled,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)", (client, "dcr", "{}", "fingerprint", 2 if case == "revision_changed" else 1, past if case == "client_expired" else future, int(case == "client_disabled"), now, now))
            db.execute("INSERT INTO works_installations(installation_id,account_id,label,status,created_at) VALUES(?,?,?,?,?)", (installation, account, "fixture", "disabled" if case == "installation_disabled" else "active", now))
            family_expiry = past if case == "family_expired" else future
            refresh_expiry = past if case == "refresh_expired" else future
            db.execute("INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at,revoked_at) VALUES(?,?,?,?,?,?,?,?,?)", (family, account, client, 1, "resource", installation, now, family_expiry, now if case == "revoked" else None))
            db.execute("INSERT INTO works_refresh_tokens(token_digest,family_id,expires_at,used_at) VALUES(?,?,?,?)", (digest, family, refresh_expiry, now if case == "used" else None))
            originals[family] = (family_expiry, refresh_expiry)
        db.executescript((MIGRATIONS / "0030_works_long_lived_refresh.sql").read_text())
        snapshots = db.execute("SELECT family_id, old_family_expires_at, old_refresh_expires_at FROM works_refresh_lifetime_migration").fetchall()
        self.assertEqual(snapshots, [("f-valid", future, future)])
        for family, old in originals.items():
            actual = db.execute("SELECT f.expires_at,r.expires_at FROM works_families f JOIN works_refresh_tokens r USING(family_id) WHERE f.family_id=?", (family,)).fetchone()
            self.assertEqual(actual, (LONG_EXPIRY, LONG_EXPIRY) if family == "f-valid" else old, family)


if __name__ == "__main__":
    unittest.main()
