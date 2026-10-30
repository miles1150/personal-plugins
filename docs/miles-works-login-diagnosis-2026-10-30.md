# Google sign-in investigation

The Mac reported a working homepage and health endpoint, then “A valid credential is required” when starting Google sign-in. Google callback registration was reported saved. Successful Google authentication remains unverified.

Read-only production evidence at approximately 04:10 UTC:

- The control database contained one unconsumed login attempt and no accounts or sessions. Only aggregate counts and expiry timestamps were queried; no OAuth state, codes, cookies or credentials were read.
- Aggregated structured error logs included `GET /account/session` / `UNAUTHENTICATED`. No `/account/login` or callback error appeared in that interval. This does not prove that the browser received a successful login response or reached Google.
- Worker-level query-string redaction remained enabled; invocation logs and traces remained disabled.

Reproduced UI defects: a normal signed-out session response appeared as an error; the general pending-action guard silently ignored login clicks during initialization; stale errors remained visible during login; requests had no deadline; expired sessions did not restore the sign-in button.

The account and keys pages now distinguish signed-out state, give login its own duplicate guard, cancel or invalidate stale initialization, show immediate progress, and abort requests after 15 seconds with retry available. The deadline includes response-body parsing. These scripts are embedded in the Worker; the static dashboard assets do not change.

The remaining Mac acceptance should record only request status/error labels, whether a login cookie was set (boolean), and the authorization destination hostname. Do not copy full authorization URLs, query strings, response bodies or cookies into logs or chat. Confirm reaching Google, returning to `/account`, and an authenticated session; then resume message and keys acceptance. A remaining external navigation timeout must be diagnosed separately from the corrected UI errors.

## Follow-up after the UI fix

Commit `248cd24` passed 29 account/keys UI tests and the Wasm build. A cloud version-upload call first stopped with `aborted before dispatch`; the complete nine-version list confirmed no candidate was created. One authorized retry reached Cloudflare but returned HTTP 415 (`Content-Type must be one of: multipart/form-data`). No further cloud upload retry was made.

The Mac subsequently deployed version `49a1f165-4afc-4b28-85cf-5258732fc74a`. A read-only API check confirmed deployment `a1ad13a3-1568-4a4f-ab28-a71e9e12ed82`, created at 04:28:19 UTC, serving that version at 100%. Worker-level redaction remained enabled. A new unconsumed login attempt was created at 04:28:59 UTC; accounts and sessions remained zero. Error aggregates since deployment contained only the signed-out session response.

The Mac then reported that the original browser tab was gone, preventing response inspection; opening Google timed out, and TLS connection attempts to both Google and the new site failed. This establishes a local network obstacle, not the sole cause of the original failure. No network settings or credentials were changed.

Independent follow-up checks passed:

- 11 OIDC tests: fixed authorization endpoint, PKCE/nonce, signed identity claims, Google key rotation/cache, bounded transport/body reads and no credential forwarding on redirects.
- 15 account tests: cookie/state binding, one-time callback, actual local HTTP login/session round trip, origin checks and owner restriction.
- A one-off headless Chromium check using the current page scripts/styles and the server's CSP, with all requests intercepted by synthetic fixtures: an ordinary click sent exactly one same-origin login request, navigated to the synthetic Google page, and carried the Secure/HttpOnly/SameSite=Lax login cookie on top-level return. This checks browser mechanics, not real Google or deployed configuration.

No additional production code defect was established. Resume real browser evidence collection after network recovery; do not replay an expired authorization URL or previously consumed callback. Live Google login and the subsequent business acceptance remain incomplete.
