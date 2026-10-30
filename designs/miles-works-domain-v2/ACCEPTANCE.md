# V2 implementation and release status

Status: implementation candidate; **not deployed and not accepted on real hosts**.

## Implemented

- Bootstrap OAuth without a hidden message identity; assistant permission checkboxes and explicit client mounting.
- Per-request assistant context, independent message/key read/update/create permissions, live authorization checks and refresh/revocation handling.
- Work sessions, optional defaults, explicit recipients, atomic replacement on removal, conversation ownership, claims, renewals, generation fencing and unknown-result handoff.
- Independent bridge OAuth, receiver proof, multiple-session supervisor, reference-only polling and persistent notification recovery.
- Encrypted message attachments and account-shared versioned material snapshots through MCP.
- Chinese/English account and dashboard flows, contextual mount review, actual bridge poll history and simpler visual hierarchy.
- Plugin 0.7.0 source and reproducible bridge packaging. Existing 0.6.0 marketplace distribution remains unchanged until a new release is verified.

## Verification evidence

- Native Rust: 128 library tests and all integration targets passed; all-target Clippy with warnings denied and formatting passed before the final test-only additions.
- Native MCP integration exercises actual OAuth/token handling, browser-session/CSRF mounting, multiple assistants, messages, attachments, materials, key permission combinations and revocation. It uses synthetic local identity, not Google.
- Python service checks: 32 passed. Plugin/marketplace checks: 19 passed.
- Browser logic: 14 tests passed. Chinese/English desktop/mobile screenshots used synthetic HTTP responses; see the dashboard frontend review.
- Bridge: 36 tests reported by its implementation reviewer; host execution and Google are not covered by those tests.
- All 26 migrations applied in empty SQLite. A synthetic legacy database upgraded with its account/Actor/Consumer/OAuth records intact and integrity/foreign-key checks clean.
- Isolated real D1: migration 0022 schema/triggers, conversation CAS, concurrent delivery claim, stale completion rejection. Temporary database `d3453086-ec1e-41e5-a498-eed6aa19ce95` was deleted after verification. This is not complete Rust-on-D1 acceptance.
- Wasm release build and Wrangler 4.111.0 production-profile dry-run passed. Four direct Worker test scripts passed against the compiled Worker, including v2 DCR/PKCE/bootstrap rejection. These use synthetic local identity, not Google or deployed D1. No version was uploaded or activated.
- Unified MCP material boundary: 128 KiB decoded content round-trips; 128 KiB + 1 is rejected, and a JSON-RPC body over 256 KiB is rejected.

## Deployment blocker

The current managed cloud executor has no Cloudflare CLI credential or outbound identity, and its enforced network policy allows package-host destinations, not the Cloudflare API or production site. The Cloudflare connector can inspect settings, but its request interface cannot supply the temporary Authorization JWT required by Workers Static Assets uploads. Previous asset-upload failures were not retried or routed around.

To resume deployment, attach an authorized environment with working Wrangler access (for example the existing Mac mini), or configure the cloud environment's Cloudflare credential and permitted API/site destinations through its supported configuration workflow. Do not paste credentials into chat. This workspace contains no exported production secrets.

Last verified production remains Worker `miles-works-production`, version `49a1f165-4afc-4b28-85cf-5258732fc74a`, with query-string redaction enabled. Preserve the five existing secret bindings, DB, KEYS_DB, OBJECTS, owner pin, domain and rate limits.

## Release sequence

1. Start from the reviewed commit and existing production configuration; compare remote configuration again. Do not recreate databases or encryption keys.
2. Exercise all new migrations and actual Worker/R2 flows on isolated resources, including current legacy-data compatibility and failure recovery.
3. Apply only pending migrations to both production bindings after compatibility checks. No destructive down migration.
4. Upload a candidate using `wrangler versions upload --strict --config wrangler.unified.production.toml`. Do not remove `--strict` to bypass a remote conflict; compare and reconcile first.
5. Verify candidate assets, bindings and secret names. Query-string redaction is a Worker-level setting; verify it before and after activation. Wrangler 4.111.0 does not recognize it as a configuration-file field.
6. Activate the verified candidate, check public routes and metadata, then let the user complete Google and client approval personally.
7. Verify real message → intended session → claim → visible effect → completion; verify synthetic key create/read/update and permission denial, refresh and revoke. Then publish the verified plugin distribution for each supported platform.

## Remaining limits

- No production v2 activation, new real Google connection, Mac mini execution, Claude model-turn calls or Grok acceptance in this run.
- No web attachment controls or standalone vault-management page; supported data operations are through MCP.
- Attachment limit is reported by reservation (193,536 bytes under the unified profile). Expired unattached objects are reclaimed on a later authorized reservation, not while the service is idle.
- Material snapshots are bounded; no automatic old-Space migration or permanent-history deletion tool.
- Control DB and keys DB do not provide atomic cross-database revocation. An already-authorized in-flight operation can finish; later checks reject changed authority.
- Explicit family revocation updates mount/routing eligibility. Natural expiry or client-registration invalidation has no background cleanup job; authorization still rejects invalid credentials.
- Rollback cannot make an old binary understand v2 sessions. Stop v2 dispatch and preserve unknown/in-flight work before selecting a compatible recovery version.
