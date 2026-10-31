# Miles Works 0.7.0

Install the plugin and follow the bundled Waypost setup guide for direct MCP login and assistant mounting. The optional background bridge uses its own OAuth installation and separately approved assistant attachment. Its wake adapter currently supports Codex; Claude supports direct MCP.

Preserve existing bridge credentials and state directories. A new OAuth authorization creates a new installation; moving an existing v2 journal to a different installation is not supported. An installation mismatch stops processing and retains the journal, including accepted and unknown attempts. Do not delete state or automatically replay unknown attempts. See `skills/waypost/references/setup.md` for safe handling and setup steps.

Package checks and synthetic OAuth tests do not establish successful real Google sign-in, client mounting, or end-to-end message processing on a user's host. Verify those separately. Legacy and v2 receivers must not run concurrently for the same work.
