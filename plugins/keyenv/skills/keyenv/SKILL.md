---
name: keyenv
description: Use the local keyenv CLI to select encrypted credentials by redacted metadata and inject literal values into an explicitly requested subprocess. Also supports user-directed vault setup, backup, recovery, and Drive synchronization.
---

# Keyenv

Use an installed `keyenv`, or the explicitly selected repository build. Never use `keyenv-lab` for actual credentials; its fixture key is public.

Read `tools/keyenv/README.md` for this version's supported commands before operating. The current CLI does not provide a help command. Use `list` and `inspect` to identify entries through metadata only. Resolve ambiguous names with the user. Never request, display, read back, or copy secret values into chat, command arguments, logs, or Git.

Sensitive input belongs in a local terminal controlled by the user. Explain the exact local window and prompt. Do not simulate typing passwords, verification codes, or credential values. Noninteractive refusal is a request for local user interaction, not permission to bypass it.

Prefer `run` with an explicit entry and executable argument array. Values are literal environment variables; do not interpolate them into shell commands. Explain when existing variables will be replaced. Do not run unrelated programs or modify accounts. `export` creates plaintext and requires explicit user authorization for the destination.

For Drive setup, explain the target Google account and `drive.file` scope. Obtain action-time confirmation before creating OAuth credentials, accepting policies, or granting persistent access. The user personally completes authentication and verification. Do not retrieve verification messages. Tokens remain in process memory in the current implementation.

Treat pending writes, conflicts, disabled entries, corrupted caches, and unavailable authentication as explicit states. Never choose a conflict winner automatically. Offline cache reads cannot establish freshness. Preserve encrypted backups and local modifications. Do not push, publish, revoke credentials, or install globally without authorization.

This is a portable skill source, not proof of native plugin registration or completed real-account acceptance.
