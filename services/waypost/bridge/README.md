# Miles Works bridge


For the packaged plugin's first-time setup, use its bundled `skills/waypost/references/setup.md` and v3 configuration templates. The v2 supervisor can target Codex or a webhook integration with its own independently mounted execution session. A successful package check does not verify login, mounting, pairing or actual processing. Grok Bot webhook authentication and JSON transport are documented by its provider; its independent MCP confirmation and end-to-end use here remain unverified.

The v2 supervisor uses its own OAuth installation and a separate assistant
attachment. Its credential has the assistant's full approved permissions; do not
copy a Codex or other host credential. The user signs in and approves the
bridge attachment separately. Register each receiver session from its own mounted
execution client with `works_sessions_register`, using `capability: "wakeable"`,
then configure the returned session ID. The bridge does not register or claim
work on behalf of the execution client.

Use a new private state directory and a mode 0600 configuration. The OAuth
credential parent directory must be mode 0700. The existing PKCE and safe refresh
logic remains in use. The first configured session is pinned to this state
directory; keep it stable and append other sessions as needed.

```json
{
  "config_version": 3,
  "base_url": "https://example.com",
  "state_dir": "/home/me/.local/state/works-bridge-v2",
  "oauth": {"credentials_file": "/home/me/.config/works/bridge-oauth.json", "client_id": null, "redirect_port": 0},
  "sessions": [
    {"assistant_id": "assistant_id", "session_id": "session_id", "thread_id": "12345678-1234-1234-1234-123456789abc", "codex_path": "/absolute/path/to/codex", "codex_remote": null, "codex_home": null}
  ]
}
```

```sh
waypost-bridge --config /absolute/path/v2.json login
waypost-bridge --config /absolute/path/v2.json mount-request ASSISTANT_ID
waypost-bridge --config /absolute/path/v2.json mount-status REQUEST_ID
waypost-bridge --config /absolute/path/v2.json pair SESSION_ID
waypost-bridge --config /absolute/path/v2.json once
waypost-bridge --config /absolute/path/v2.json run
waypost-bridge --config /absolute/path/v2.json status --items
waypost-bridge --config /absolute/path/v2.json import-legacy /absolute/path/old-state-dir
```

`pair` sends a challenge to the configured receiver; that session must confirm
it using its own attached connection. A queue receipt is not proof. The
supervisor polls every 30 seconds, separately for each paired session, and
rechecks proof and assignment before sending a reference. It never submits a
claim or a processing receipt. A wake whose outcome is unknown is not resent.
After independently checking the outcome, use `reconcile ATTEMPT_ID accepted`
or `reconcile ATTEMPT_ID blocked`. A pairing challenge is also sent only once;
after independently checking an ambiguous challenge, `pair-renew SESSION_ID`
starts a fresh proof and retains the previous pairing in the journal. Expired
proofs require the same explicit renewal. Stop the legacy bridge before importing its state; the import opens the old
database read-only, copies its references into quarantine, and does not infer
v2 delivery IDs. Retain the old state and do not run legacy and v2 receivers
for the same work.

Version 0.7.1 no longer runs legacy bridge configurations. It rejects them before opening their credentials or journal. Preserve the old files; use a new v2 configuration and `import-legacy` only after stopping the old receiver. Imported references remain quarantined and cannot be automatically replayed.

Do not infer real-host acceptance from Worker deployment, unit tests, or a queue receipt. Record actual Google login, each independent client mount, pairing, message receipt and processing separately for the version being released. A new authorization creates a new installation; cross-installation journal transfer is not supported. Preserve the credential and journal, inspect unknown results, and stop the receiver if the original credential cannot be used.
