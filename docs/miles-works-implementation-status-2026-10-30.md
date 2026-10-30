# Miles Works implementation checkpoint

Base: live remote main `1992d19dd521a1069880c7b49fdb99df2a201e87`, rechecked on 2026-10-30. The new application and assets are deployed; the current package is 0.6.0 with one MCP; live host OAuth/business acceptance remains pending. Legacy services, connections and data are preserved. Existing installations are not modified automatically. The current source retains only Miles Works; see the marketplace guide for the verified distribution receipt.

## Implemented

- Optional unified Worker: one OAuth resource and MCP catalog, independent message/key capability grants, separate control and vault D1 bindings and independent encryption keys. Full read/change/create preset with explicit consent and granular alternative.
- Bridge browser PKCE login, private atomic credentials, refresh recovery, receiver identity proof before dispatch, preserved legacy token configuration and notification journal.
- Nonrenewable bounded receiver claims, fencing and processed receipts. Notification acceptance, identity proof and completed processing remain distinct states. External business effects still need idempotency.
- Shared website/dashboard/account/keys design foundations. Advanced identity and permission fields remain available without dominating the default journey.
- Fail-closed personal production configuration, current Google owner/issuer guards, explicit combined DCR/CIMD scope handling, preflight and migration/rollback guide.
- Updated receiver skills and bridge artifact provenance; original upstream commit is no longer presented as the source of changed monorepo bytes.

## Verification

- Final backend library suite: 119 passed. Full native integration, strict Clippy, Wasm checks and container acceptance passed in CI run 41012296936. Its Worker OAuth discovery check still expected the old 11-tool catalog; corrected to 14 and passed locally, with full CI rerun pending.
- Bridge: 31 tests and strict Clippy passed.
- UI: 35 tests passed; account/keys subset (18) reran after simplification. Desktop and 320px Chromium checks passed with synthetic API responses. Static/i18n checks passed.
- Final Wasm build and Wrangler 4.111.0 dry-run passed: approximately 3.6 MiB raw / 1.18 MiB compressed Worker upload. Dry-run used local deployment configuration, not live bindings.
- `tests/unified_worker.mjs` passed against the final compiled Worker in Workerd, two isolated D1 stores and R2: synthetic Google browser OAuth, DCR/PKCE consent, combined catalog, encrypted message roundtrip, keys CRUD and key-only revocation preserving messages. Zero unexpected outbound calls. Included in the existing Worker CI job.
- Historical isolated host experiments were rerun: Codex CLI/app-server 5 checks, Claude Code 4 checks and 10 isolation boundaries. These used a prototype service; they do not prove the production implementation on those hosts.

## Live acceptance and remaining work

The Mac task backed up and uploaded all five required deployment secrets. Cloud API checks confirmed names and bindings without reading values. The new `works.miles1150.top` domain is active; the verified Google owner restriction remains configured.

Current application version: `49a1f165-4afc-4b28-85cf-5258732fc74a`, deployed at 100% through `a1ad13a3-1568-4a4f-ab28-a71e9e12ed82`. The Mac coordinator now reports real Google login succeeded, Personal Space was created and selected, and its inbox opened. These are real browser onboarding results; they do not establish MCP authorization or message/key processing. The earlier TLS failure was an observed temporary obstacle, not a confirmed sole root cause.

The login fix (`248cd24`) passed 29 account/keys UI tests, a Wasm build, and isolated Chromium navigation/cookie checks. Follow-up OIDC/account tests passed 26 checks. See the [login evidence](miles-works-login-diagnosis-2026-10-30.md) and [deployment receipt](miles-works-cloudflare-setup-2026-10-30.md).

The user's latest instruction is to install the marketplace/plugin and personally experience login. Installation is authorized; do not complete login or create a persistent Codex grant on the user's behalf. Production model tool calls, synthetic key CRUD, message send/read/processed receipts, refresh/revoke, bridge pairing and Grok/Claude acceptance remain open. Historical plugin 0.5.0 distribution remains recoverable at `8bf0b376f1c97b5d78dcbaeeb4e7c5d3b749f458`. Version 0.6.0 removes the three redundant plugin wrappers and marketplace entries, preserving their skills, backend services, user data and credentials. Device uninstall remains separate; no user login is performed by the release job. See the [deployment guide](../services/waypost/docs/unified-deployment.md) and [integrated plan](miles-works-integrated-plan-2026-10-30.md) for migration/deletion tasks.

Known scope limits: fresh vault only; no silent import of old ciphertext or identities; initial unified onboarding requires a message identity; notification-only scope and unified device grant are not implemented. One Worker can access both databases. Cross-database revocation cannot cancel operations already authorized in flight.

## Next controlled Mac acceptance

This is a prepared sequence, not permission to create a connection. The user will complete login and consent personally. Start tool acceptance only after that connection is available and its granted permissions have been checked.

1. Connect that host to `https://works.miles1150.top/mcp`; preserve legacy host entries. Record host version, Worker version and consent scope without codes/tokens. Check `works_identity` and `waypost_describe` agree on the selected Space, Actor and Consumer before sending anything.
2. In an actual model turn, send a clearly labelled synthetic message with a persisted request ID. Read it through the intended recipient, durably record it, then complete the claim/processed receipt. Match message ID and recipient identity. A self-message checks tools but does not prove another Agent or bridge was awakened.
3. Use one uniquely named synthetic key entry; never fetch an existing production key for acceptance. Call `create_entry`, `fetch_entry`, `update_entry` and fetch again, comparing only in private execution memory and reporting booleans. Do not echo returned fields in conversation. Preserve the test entry for later approved cleanup.
4. Reopen the host and exercise its ordinary refresh path, confirming the same identity and no scope expansion. Do not manually edit persistent tokens or production database expiry to simulate refresh; record a missing refresh observation as pending.
5. Test key-only and full-connection revocation only within an explicitly agreed disposable connection/revocation scope. Confirm key denial with messages retained, then full denial; do not revoke the user's ongoing connection merely to finish a checklist.
6. Treat bridge OAuth/pairing, receiver fencing, Claude/Grok model calls and legacy migration as separate remaining checks. The user subsequently requested the single-resource plugin release for personally testing login. Do not remove old connections or claim business acceptance based solely on a catalog listing.
