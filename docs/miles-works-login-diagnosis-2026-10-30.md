# Google sign-in investigation

The Mac reported a working homepage and health endpoint, then “A valid credential is required” when starting Google sign-in. Google callback registration was reported saved. Successful Google authentication remains unverified.

Read-only production evidence at approximately 04:10 UTC:

- The control database contained one unconsumed login attempt and no accounts or sessions. Only aggregate counts and expiry timestamps were queried; no OAuth state, codes, cookies or credentials were read.
- Aggregated structured error logs included `GET /account/session` / `UNAUTHENTICATED`. No `/account/login` or callback error appeared in that interval. This does not prove that the browser received a successful login response or reached Google.
- Worker-level query-string redaction remained enabled; invocation logs and traces remained disabled.

Reproduced UI defects: a normal signed-out session response appeared as an error; the general pending-action guard silently ignored login clicks during initialization; stale errors remained visible during login; requests had no deadline; expired sessions did not restore the sign-in button.

The account and keys pages now distinguish signed-out state, give login its own duplicate guard, cancel or invalidate stale initialization, show immediate progress, and abort requests after 15 seconds with retry available. The deadline includes response-body parsing. These scripts are embedded in the Worker; the static dashboard assets do not change.

The remaining Mac acceptance should record only request status/error labels, whether a login cookie was set (boolean), and the authorization destination hostname. Do not copy full authorization URLs, query strings, response bodies or cookies into logs or chat. Confirm reaching Google, returning to `/account`, and an authenticated session; then resume message and keys acceptance. A remaining external navigation timeout must be diagnosed separately from the corrected UI errors.
