# Fresh Miles Works deployment

The unified Worker serves one `/mcp` resource. `DB` holds accounts, OAuth connections
and messages; `KEYS_DB` holds a separate account mapping, local key grants and
encrypted entries. Both bindings and both keyrings are accessible to the same
Worker process. This is a storage and authorization boundary, not process isolation.

Keep `waypost-production`, `waypost-keys-production`, their origins, credentials,
databases, keyrings and objects intact. This first deployment creates an empty
vault; it does not migrate old entries. Account IDs are encryption AAD: never copy
ciphertext under new IDs or join accounts by email. Mapping uses verified OIDC
`(issuer, subject)` only.

## Preflight

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
the same verified migrations. Do not run these commands with either legacy
configuration. Provision/list secret **names** through the chosen deployment
channel before activation; no old-secret export is needed. When staging is
accepted, deploy the reviewed fresh configuration and record Git SHA, Worker
version, both database IDs and applied migration names. Keep the custom domain
and plugin pointing at legacy resources until the new endpoint has completed its
own real Google/OAuth and synthetic-message/key acceptance.

Allow the new origin's required `/mcp`, `/.well-known/oauth-*`, `/oauth/*`,
`/account/*`, `/dashboard/*`, `/v0/*` and blob upload/download routes through its
edge rules. Do not reuse a narrow legacy path allowlist without review. `/ready`
requires account-owner authentication and checks **both** database migration
registries and the R2 binding. `/health` is liveness, not proof of successful IdP,
OAuth or vault operation.

## Acceptance and rollback

Initial OAuth defaults to `waypost`; it requires a message identity. Key access
is off unless explicitly requested and approved. Read-all includes future
entries; selected entries do not. Write and create are separately selected and
still intersect `keys:write`; created entries are readable only with
`keys:read`. Browser PKCE is supported; unified device authorization and key-only
initial onboarding are intentionally unavailable in this slice.

Prove on deployed D1: message-only cannot list/fetch keys; read-only cannot
create/change them; exact-entry grants cannot read a future entry; a full
connection revoke rejects both capabilities; refresh rotates without expanding
scope; revoking only keys preserves messages, and reducing messages preserves
keys. Use synthetic values. Separately complete receiver identity proof,
claim/fencing/receipt and real-host acceptance; SQLite tests are not D1 or device
acceptance.

Every new key call rechecks the parent OAuth family, issuer/subject, audience,
client revision, token expiry, capability/version and local key grant. Sensitive
writes recheck parent authority before a locally guarded transaction. Revocation
across the two databases is not atomic: already-authorized operations can finish,
and returned secrets cannot be recalled. Changing the owner pin guards existing
unified credentials but is not durable revocation: restoring a pin can restore
an otherwise-live connection. Explicitly revoke old families when replacing an
owner. Legacy credentials retain their existing lifecycle.

Rollback routes/installations to the still-running legacy Workers, or select a
previous version of the **new** Worker compatible with its additive schema.
Keep new tables and evidence; do not use destructive down migrations or delete
old resources to roll back. The old vault remains available through its original
resource until a separately reviewed identity/AAD-preserving migration exists.
