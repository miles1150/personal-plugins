# Miles Works 0.7.2 (unreleased)

Existing valid browser sessions can approve bootstrap and mounts without an extra 15-minute local-session-age gate; each client and mount still requires explicit user approval. Denials, revocation and read-only dashboard operations also work with a valid older session. Other permission expansion keeps its existing checks; this does not implement verified step-up authentication.

User-approved A2A delegation is separate from client mounts and message content. The owner selects the receiving client, trusted sender assistant and actions; receivers check the current grant and verified source session before autonomous work and replies. Existing messages without a verified source session do not silently gain delegation. This version requires a compatible Worker and additive database migrations before the feature can work; publishing the plugin alone does not activate it. Recheck permissions and leases, and follow host-level security controls.

# Miles Works 0.7.1

The v2 bridge adds a reference-only webhook receiver with independent authentication and pairing, including an explicitly opted-in local loopback option. Receivers must implement the bundled v2 event and confirmation contract; Grok Bot end-to-end behavior has not been verified. HTTP acceptance is not task completion. Preserve all existing credentials and journals, and never run old and new receivers for the same work.

The server candidate retires legacy MCP and public `/v0` routes on the unified Worker with HTTP 410; this requires a separate server rollout and can interrupt old clients. It preserves stored data and does not retire separately deployed legacy services. The old Space/Actor/Consumer bridge runtime is retired. Old configurations are rejected before opening their journal. Stop any old receiver and use a new v2 configuration; the `import-legacy` command copies references into quarantine without deleting or replaying the old journal.

# Miles Works 0.7.0

Install the plugin and follow the bundled Waypost setup guide for direct MCP login and assistant mounting. The optional background bridge uses its own OAuth installation and separately approved assistant attachment. Its wake adapter currently supports Codex; Claude supports direct MCP.

Preserve existing bridge credentials and state directories. A new OAuth authorization creates a new installation; moving an existing v2 journal to a different installation is not supported. An installation mismatch stops processing and retains the journal, including accepted and unknown attempts. Do not delete state or automatically replay unknown attempts. See `skills/waypost/references/setup.md` for safe handling and setup steps.

Package checks and synthetic OAuth tests do not establish successful real Google sign-in, client mounting, or end-to-end message processing on a user's host. Verify those separately. Legacy and v2 receivers must not run concurrently for the same work.
