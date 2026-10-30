# Single MCP feasibility fixture

This is an isolated experiment, **not production code**. It binds only loopback,
uses synthetic identity/keys, and never calls a production service or model API.
Real Codex and Claude Code host configuration is isolated in fresh private
directories and deleted after each run. No existing user credentials are read.

The fixture has six tools on one MCP resource: identity, message send, key list,
key fetch, key update, and a pending connection proposal. Authorization is checked
on every direct call. It includes discovery, DCR, S256 PKCE, code redemption,
refresh rotation/replay detection, per-entry grants and family revocation.
Consent is deliberately auto-approved as a synthetic fixture, with no real IdP.
Tokens are in memory; no TLS, durable storage or complete security hardening.
Do not publish, tunnel, deploy or use it with real secrets.

```sh
# Python-only boundary verification, no dependencies:
python3 verify.py

# Pinned Claude Code 2.1.312 and official MCP TypeScript SDK 1.32.2:
npm ci --cache /tmp/works-lab-npm --no-audit --no-fund
python3 verify.py --sdk

# Requires installed `codex` on PATH; tested codex-cli 0.159.2:
python3 host_verify.py codex
python3 host_verify.py claude
```

Host tests run real CLI OAuth and health checks, using a local synthetic owner
approval. Codex additionally calls tools through app-server without a model turn.
Claude health checks send initialize/tools/list; no Claude model tool call is
claimed. The driver provides a PTY for Claude's headless callback prompt.

`boundary-results.json`, `codex-results.json`, `claude-results.json` record the
actual results. They include protocol method names and pass/deny events, never
tokens or key/message values. A `partial` result is not a pass: inspect `blocker`.
The scripts exit normally to preserve partial evidence; verify `status` in JSON.
Random client/family identifiers and port numbers vary on every run.

`lab.py` can also be run manually; it prints only the loopback origin/resource.
Stop with Ctrl-C. The fixture is independent of Waypost native/Worker code: its
success proves a transport/host/authorization design, not production integration.

See [the full Chinese report](../../docs/miles-works-technical-feasibility-2026-10-29.md)
and [the next-stage prompt](NEXT-PROMPT.zh.md).

## Recovery on 2026-10-30 (Asia/Singapore)

Recovered from complete thread file diffs; not verified byte-for-byte against the original Library ZIP. Results JSON files are fresh reruns. npm lockfile regenerated using the same pinned versions. repository-checks.json and the technical report are historical; see rerun-2026-10-30.json and the integrated plan for this run.
