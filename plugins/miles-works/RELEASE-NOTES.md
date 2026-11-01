# Miles Works 0.7.2 (unreleased)

Existing valid browser sessions can connect and manage current Miles Works settings without an arbitrary 15-minute session-creation-time gate. Each new client requires one explicit connection confirmation; afterwards eligible same-account assistants attach automatically, and the client inherits their enabled message and key permissions. Valid connections can refresh after long offline periods without a fixed inactivity expiry. Migration extends currently valid credentials; it does not revive already expired, revoked or disabled credentials. Account, client and assistant policy checks still apply. This does not implement verified step-up authentication.

Same-account authenticated assistants can exchange and autonomously carry out tasks without separate per-sender delegation settings. Receivers verify the source session and live assignment, preserve execution and reply receipts, and obey host-level security controls. Existing messages without a verified source session are not automatically executed as tasks. The old per-client delegation and attachment-revocation controls are retired; historical records are preserved, and a previously revoked attachment cannot be automatically recreated by the same installation. The compatible Worker retires legacy unified MCP and `/v0` endpoints with HTTP 410; legacy clients must use v2. This version requires a compatible Worker and additive database migrations; publishing the plugin alone does not activate the feature.

# Miles Works 0.7.1

The v2 bridge adds a reference-only webhook receiver with independent authentication and pairing, including an explicitly opted-in local loopback option. Receivers must implement the bundled v2 event and confirmation contract; Grok Bot end-to-end behavior has not been verified. HTTP acceptance is not task completion. Preserve all existing credentials and journals, and never run old and new receivers for the same work.

The server candidate retires legacy MCP and public `/v0` routes on the unified Worker with HTTP 410; this requires a separate server rollout and can interrupt old clients. It preserves stored data and does not retire separately deployed legacy services. The old Space/Actor/Consumer bridge runtime is retired. Old configurations are rejected before opening their journal. Stop any old receiver and use a new v2 configuration; the `import-legacy` command copies references into quarantine without deleting or replaying the old journal.

# Miles Works 0.7.0

Install the plugin and follow the bundled Waypost setup guide for direct MCP login and assistant mounting. The optional background bridge uses its own OAuth installation and separately approved assistant attachment. Its wake adapter currently supports Codex; Claude supports direct MCP.

Preserve existing bridge credentials and state directories. A new OAuth authorization creates a new installation; moving an existing v2 journal to a different installation is not supported. An installation mismatch stops processing and retains the journal, including accepted and unknown attempts. Do not delete state or automatically replay unknown attempts. See `skills/waypost/references/setup.md` for safe handling and setup steps.

Package checks and synthetic OAuth tests do not establish successful real Google sign-in, client mounting, or end-to-end message processing on a user's host. Verify those separately. Legacy and v2 receivers must not run concurrently for the same work.
