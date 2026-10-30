# Miles Works implementation checkpoint

Base: live remote main `1992d19dd521a1069880c7b49fdb99df2a201e87`, rechecked on 2026-10-30. This branch is unreleased. Existing production services, connections and data have not been changed. Installed plugin configuration still points to the two existing resources until a new endpoint passes deployment acceptance.

## Implemented

- Optional unified Worker: one OAuth resource and MCP catalog, independent message/key capability grants, separate control and vault D1 bindings and independent encryption keys. Full read/change/create preset with explicit consent and granular alternative.
- Bridge browser PKCE login, private atomic credentials, refresh recovery, receiver identity proof before dispatch, preserved legacy token configuration and notification journal.
- Nonrenewable bounded receiver claims, fencing and processed receipts. Notification acceptance, identity proof and completed processing remain distinct states. External business effects still need idempotency.
- Shared website/dashboard/account/keys design foundations. Advanced identity and permission fields remain available without dominating the default journey.
- Fail-closed personal production configuration, current Google owner/issuer guards, explicit combined DCR/CIMD scope handling, preflight and migration/rollback guide.
- Updated receiver skills and bridge artifact provenance; original upstream commit is no longer presented as the source of changed monorepo bytes.

## Verification

- Final backend library suite: 119 passed. Full native integration suite passed before the final focused production/DCR hardening; affected library tests reran afterward.
- Bridge: 31 tests and strict Clippy passed.
- UI: 35 tests passed; account/keys subset (18) reran after simplification. Desktop and 320px Chromium checks passed with synthetic API responses. Static/i18n checks passed.
- Final Wasm build and Wrangler 4.111.0 dry-run passed: approximately 3.6 MiB raw / 1.18 MiB compressed Worker upload. Dry-run used local deployment configuration, not live bindings.
- `tests/unified_worker.mjs` passed against the final compiled Worker in Workerd, two isolated D1 stores and R2: synthetic Google browser OAuth, DCR/PKCE consent, combined catalog, encrypted message roundtrip, keys CRUD and key-only revocation preserving messages. Zero unexpected outbound calls. Included in the existing Worker CI job.
- Historical isolated host experiments were rerun: Codex CLI/app-server 5 checks, Claude Code 4 checks and 10 isolation boundaries. These used a prototype service; they do not prove the production implementation on those hosts.

## Activation blockers and remaining acceptance

Cloudflare management API access is available, but existing Worker secrets cannot be exported through it. The existing keys service has one active verified Google account; its subject was checked without reading secret values and need not be supplied again. New-origin Google Web OAuth configuration and callback registration remain missing. The candidate configuration uses `https://works.miles1150.top/account/callback`; domain selection and actual Google client setup must be resolved before activation.

No live Google login, production deployment, Grok model call, Codex/Claude production refresh/revoke, or Mac mini end-to-end verification is claimed. The current environment has no configured Mac/dot execution connection. See [deployment guide](../services/waypost/docs/unified-deployment.md) and [integrated plan](miles-works-integrated-plan-2026-10-30.md) for acceptance and the explicit legacy migration/deletion backlog.

Known scope limits: fresh vault only; no silent import of old ciphertext or identities; initial unified onboarding requires a message identity; notification-only scope and unified device grant are not implemented. One Worker can access both databases. Cross-database revocation cannot cancel operations already authorized in flight.
