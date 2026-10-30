# Miles Works unified deployment

The unified Worker serves one `/mcp` resource. `DB` holds accounts, OAuth connections
and messages; `KEYS_DB` holds a separate account mapping, local key grants and
encrypted entries. Both bindings and both keyrings are accessible to the same
Worker process. This is a storage and authorization boundary, not process isolation.

Current hosted origin: `https://works.miles1150.top`; MCP resource:
`https://works.miles1150.top/mcp`; Worker: `miles-works-production`.
The standalone Waypost and Keys services and their old data resources were retired
with the owner's explicit authorization to discard unmigrated data without backup.
They are not available for fallback, migration or token reuse.

## Existing-service updates

Reuse the reviewed current Worker and its DB, KEYS_DB, OBJECTS and secret bindings.
Do not recreate resources, generate replacement encryption keys or execute the
fresh-install steps below against existing data. Prepare a candidate with the
current configuration, verify static assets and schema compatibility, then activate
it through the approved deployment process. Preserve query-string redaction.

Account IDs are encryption AAD: never copy ciphertext under new IDs or join accounts
by email. Verified OIDC `(issuer, subject)` remains the account-mapping key.

## Fresh installation only: preflight

1. Copy `wrangler.unified.production.example.toml` to the ignored
   `wrangler.unified.production.toml`. Allocate a **new** Worker, two **different
   fresh D1 database IDs**, a new R2 bucket and new rate limiter namespaces.
   Review those IDs against the existing deployment inventory. Runtime binding
   handles cannot detect an accidentally duplicated D1 ID.
2. Register the exact HTTPS callback `<PUBLIC_ORIGIN>/account/callback` in a real
   Google Web OAuth client. Set `WORKS_OWNER_SUBJECT` to the intended owner's
   verified Google `sub`, never an email. Signup limit 1 alone does not identify
   the owner. Production rejects a missing owner pin, fixture RSA provider,
   missing Google rotating JWKS, missing keyring or enabled operator access.
3. Provision these five **new independent** Worker secrets through secure input:
   `WAYPOST_OIDC_CONFIG`, `WAYPOST_CSRF_SECRET`, `WAYPOST_CURSOR_SECRET`,
   `WAYPOST_ENCRYPTION_KEYS`, `KEYS_DATA_KEYS`. Keep a recoverable private backup.
   Do not paste values into chat, command arguments, source control or logs.
   CSRF and cursor signers must each contain at least 32 random bytes and differ.
   The two encryption keyrings must use independently generated 32-byte keys.
4. The OIDC secret is a JSON object with `issuer` =
   `https://accounts.google.com`, `authorization_endpoint` =
   `https://accounts.google.com/o/oauth2/v2/auth`, `token_endpoint` =
   `https://oauth2.googleapis.com/token`, real `client_id` and `client_secret`,
   `google_jwks` = `true`, and `keys` = `[]`. Both encryption keyrings use
   `{"active":"k1","keys":{"k1":"<64 hex characters>"}}` with different values.
   The Worker uses fixed Google exchange/JWKS transports. Configuration validation
   does not prove Google client/callback availability; real login must still be
   accepted before the deployment is considered usable.
5. Run `python3 scripts/check_unified_config.py wrangler.unified.production.toml`.
   This only reads non-secret configuration. It validates separation/personal
   mode and rejects example placeholders; it cannot verify remote resource
   ownership, secret presence or whether the database is genuinely fresh.

## Build, initialize and activate

Run from `services/waypost`, using authenticated Wrangler 4 and `worker-build`.
The following commands mutate only the explicitly selected fresh resources:

```bash
npx wrangler d1 migrations apply DB --remote --config wrangler.unified.production.toml
npx wrangler d1 migrations apply KEYS_DB --remote --config wrangler.unified.production.toml
npx wrangler deploy --dry-run --config wrangler.unified.production.toml
```

Apply the full `migrations/` directory to **both** databases. Most message tables
in `KEYS_DB` remain empty; schema parity lets current Rust storage adapters reuse
the same verified migrations. Do not run these commands with a local compatibility profile. Provision/list secret **names** through the chosen deployment
channel before activation; no old-secret export is needed. When staging is
accepted, deploy the reviewed fresh configuration and record Git SHA, Worker
version, both database IDs and applied migration names. For a fresh installation, point its domain and plugin at the reviewed new resource
only after real Google/OAuth and synthetic-message/key acceptance. Never redirect
old tokens or retired service addresses to the new resource.

Allow the new origin's required `/mcp`, `/.well-known/oauth-*`, `/oauth/*`,
`/account/*`, `/dashboard/*`, `/v0/*` and blob upload/download routes through its
edge rules. Do not reuse a narrow legacy path allowlist without review. `/ready`
requires account-owner authentication and checks **both** database migration
registries and the R2 binding. `/health` is liveness, not proof of successful IdP,
OAuth or vault operation.

## Acceptance and rollback

New OAuth clients default to `works:client`. This only establishes the client
installation and permits assistant draft/mount requests; it grants no message or
key access. The owner reviews assistant permissions and client attachment in the
account UI. Assistant policies independently grant messages, key read, key update,
and key create. All mounted clients inherit that assistant policy. Multiple
assistants require explicit per-call `assistant_id`; there is no global current
assistant. Existing explicitly scoped legacy credentials retain their old checks.

Acceptance must cover bootstrap denial, per-request assistant context, mounting,
policy shrink, refresh/revoke, and separate key permission combinations. Messages
require session registration and a default or explicit target; notification is not
processing. Verify claims, renewal, fencing and completion independently of wake.
A disconnected default must not silently keep receiving new assignments.

V2 provides encrypted message attachments and account-shared material snapshots
through MCP. Attachments currently use bounded base64url JSON uploads (193,536
bytes under the unified profile); material snapshots contain up to 64 items and
128 KiB decoded content. These are not unlimited binary upload endpoints or
automatic legacy-data migration. Overdue unattached objects are reclaimed during
later authorized reservation calls, not by an idle-time scheduler. Web attachment
controls and a standalone vault-management page are not implemented. Do not claim
full acceptance until the real Worker, R2 and actual hosts have been verified.

Every new key call rechecks the parent OAuth family, issuer/subject, audience,
client revision, token expiry, capability/version and local key grant. Sensitive
writes recheck parent authority before a locally guarded transaction. Revocation
across the two databases is not atomic: already-authorized operations can finish,
and returned secrets cannot be recalled. Changing the owner pin guards existing
unified credentials but is not durable revocation: restoring a pin can restore
an otherwise-live connection. Explicitly revoke old families when replacing an
owner. Legacy credentials retain their existing lifecycle.

Rollback selects a previous version of **miles-works-production** compatible with
its current schema and data. The retired Workers and old vault are not rollback
options. Keep current tables and evidence; do not use destructive down migrations.
If a future change is not backward compatible, prepare its own recovery procedure
before activation rather than assuming an old binary can safely read new data.
