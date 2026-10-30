# Waypost bridge

Start with the [step-by-step receiver setup](../docs/receiver-setup.md): [existing Codex thread](../docs/receiver-setup.md#codex), [webhook Agent without Codex](../docs/receiver-setup.md#webhook), or [scheduled HTTP/MCP queries](../docs/receiver-setup.md#scheduled). This document is the configuration and recovery reference.

A single-process local bridge polls a Waypost inbox and sends reference-only notifications to a configured Codex thread or webhook. It discards the message returned by the visibility check, never includes its body in the notification, and never submits Waypost receipts. Before dispatch it checks that the credential can still read the message. Non-message feed entries are recorded but not dispatched.

Build with `cargo build --release --locked --manifest-path bridge/Cargo.toml`. The resulting binary is `bridge/target/release/waypost-bridge`.

Create a private directory and a token file containing only the Waypost bearer token. Create a mode 0600 JSON configuration file, replacing every value:

```json
{
  "base_url": "https://waypost.example.com",
  "token_file": "/home/me/.config/waypost/token",
  "state_dir": "/home/me/.local/state/waypost-bridge",
  "space_id": "space_id",
  "actor_id": "actor_id",
  "consumer_id": "consumer_id",
  "thread_id": "12345678-1234-1234-1234-123456789abc",
  "codex_path": "/absolute/path/to/codex",
  "codex_remote": null,
  "codex_home": null
}
```

Use a valid connection credential bound to the configured Space, Actor and Consumer. Connections now share the same space-member capabilities; there is no read-only token or per-connection permission list. The bridge itself only reads and notifies, but its credential can also send messages and read/write shared collections and sealed attachments. A separate credential can still be independently revoked or expired; it does not reduce those capabilities. Ordinary connections cannot administer the Space or read other participants’ private message bodies. `codex_remote`, when used, is an explicit `unix:///absolute/path` URL for the existing local app-server. The socket must be owned by the current user and protected from replacement; a connection probe runs before sending. The configured thread must be its exact lowercase UUID. `codex_home` may be an absolute Codex home directory. Use a dedicated private state directory per binding; identity, service, Codex executable, home, remote and thread are pinned on first use. HTTPS is required except for loopback HTTP, intended for local testing. Redirects are disabled. TLS certificate verification uses the system trust store and respects `SSL_CERT_FILE` and `SSL_CERT_DIR` when configured for a trusted proxy or custom CA.

```sh
waypost-bridge --config /absolute/path/config.json scan
waypost-bridge --config /absolute/path/config.json once
waypost-bridge --config /absolute/path/config.json run
waypost-bridge --config /absolute/path/config.json status
waypost-bridge --config /absolute/path/config.json status --items
waypost-bridge --config /absolute/path/config.json retry-blocked
waypost-bridge --config /absolute/path/config.json reconcile
```

`run` polls after 30 seconds when new items are found and 60 seconds when idle, with up to five seconds of jitter; failures back off with jitter. Each cycle reads at most ten pages before dispatch. Empty inbox polls do not write remotely. The database and process lock live in the private state directory. The bridge must have exclusive access to it. Back up the database before replacing or deleting it: forgetting the database can cause duplicate notifications on a fresh scan.

A queue attempt is recorded as `sending` before starting Codex. An interrupted attempt or a response without exactly one valid receipt is `unknown`, and is never automatically retried. `reconcile` reads the configured thread through the official Codex app-server and accepts an unknown item only when its marker appears in a user message. A missing marker does not prove non-delivery. If you have independently established the outcome, an operator can explicitly resolve one item using `reconcile ITEM_ID accepted` or `reconcile ITEM_ID pending`; the latter permits a new send and can duplicate a notification if the first attempt actually succeeded. State stores references and queue status, not message content.

The GitHub Actions workflow builds and tests Linux x86-64 and macOS x86-64/ARM64 archives with SHA-256 files as workflow artifacts after bridge changes merge to main, or on manual dispatch. Archives include the source COMMIT. Pull requests and manual dispatch also upload a Linux x86-64 `waypost-bridge` binary, `waypost-bridge.sha256` (`sha256sum` format), and `COMMIT`. It does not publish a release.

Upgrading to the space-membership model invalidates older inbox cursors (`f2`/`f3`). The server returns `CURSOR_SCOPE_CHANGED`; the bridge resets its cursor and rescans while retaining the existing item-ID delivery journal. Keep `state_dir` unchanged so previously notified messages are not delivered again. Shared memory feed entries may now appear, but the bridge records them without waking the assistant. Sealed attachments and collections are shared within the Space; inbox messages retain their recipient routing. Existing thread membership remains a conversation constraint: to contact another space member outside an existing thread, send a new message without that thread ID.

A message that returns 404 at the visibility check is marked `blocked` and is not repeatedly checked. A cursor scope reset returns blocked items to pending; after independently resolving access, `retry-blocked` explicitly retries all blocked items.

## Running as a user service

The examples are templates; review and edit paths for your account before installing. Create the configuration and token with mode 0600, and keep the state directory private. Preserve the same state directory across restarts so delivery history and cursor are retained.

On Linux, copy `examples/waypost-bridge.service` to `~/.config/systemd/user/waypost-bridge.service`, adjust `ExecStart` if necessary, then run:

```sh
systemctl --user daemon-reload
systemctl --user enable --now waypost-bridge.service
systemctl --user status waypost-bridge.service
```

On macOS, replace `USERNAME` and both paths in `examples/org.waypost.bridge.plist`, copy it to `~/Library/LaunchAgents/org.waypost.bridge.plist`, then run:

```sh
launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/org.waypost.bridge.plist"
launchctl print "gui/$(id -u)/org.waypost.bridge"
```

The fixed-session queue and reconciliation design was informed by [Yobiru](https://github.com/chekusu/yobiru).

## Webhook adapter

A version 2 configuration can replace the legacy flat Codex fields with one `wake_adapter`. For Codex, use `{"type":"codex","thread_id":"...","codex_path":"/absolute/path/to/codex","codex_remote":null,"codex_home":null}`. The existing flat configuration and its database binding continue to work; do not mix flat and versioned adapter fields.

For a receiver implementing the generic Bearer webhook contract, use a separate private state directory and this adapter:

```json
{
  "type": "webhook",
  "url_file": "/home/me/.config/waypost-bridge/webhook-url",
  "secret_file": "/home/me/.config/waypost-bridge/webhook-secret"
}
```

Put this object in `wake_adapter` in a configuration containing `"config_version": 2` and the common service, token, state and identity fields above. Both files must be owned by the current user and private (0600). The webhook secret must differ from the Waypost token and reside in a separate file. Rotate it only for the same logical recipient; a new credential does not prove that the recipient is unchanged. The URL must be public HTTPS, without credentials, query or fragment. DNS results are checked before every delivery and, unless the opt-in below applies, pinned to that lookup. Redirects are disabled. If any standard HTTP, HTTPS or ALL proxy environment variable is set, delivery stops before sending, even when NO_PROXY is also set; the bridge cannot guarantee target pinning through that proxy. For local tests only, `"allow_loopback_test": true` allows literal 127/8 or ::1 HTTP targets; it does not allow private networks or DNS names.

Some egress gateways answer every DNS query with a placeholder such as `198.18.0.1` (`198.18.0.0/15`) and route the connection by hostname and SNI. That address is not public, so the default check rejects it. Operators who trust that resolver can opt in with `trusted_resolved_cidrs`:

```json
{
  "type": "webhook",
  "url_file": "/home/me/.config/waypost-bridge/webhook-url",
  "secret_file": "/home/me/.config/waypost-bridge/webhook-secret",
  "trusted_resolved_cidrs": ["198.18.0.0/15"]
}
```

Omit the key or set `"trusted_resolved_cidrs": []` to keep today's strict behavior. Each entry is a canonical CIDR string whose base address is the network address (host bits zero). When every answer for the webhook hostname is inside the listed ranges, the bridge does not pin those placeholder addresses. It connects by hostname so the system resolver and gateway can route the flow, then resolves again at connect time and fails the delivery if any new answer falls outside the listed ranges. TLS certificate verification stays enabled for the real hostname.

Only the listed ranges become acceptable. Loopback, RFC1918, link-local, CGNAT, documentation, and metadata addresses are still rejected unless that range is itself listed. A literal IP in the URL is never accepted just because it falls in a trusted range. HTTPS, the ban on userinfo, query, and fragment, disabled redirects, and the proxy-environment refusal are unchanged.

The configuration refuses entries that would turn the check off: prefix length `/0` (`0.0.0.0/0`, `::/0`), and any range that overlaps unspecified `0.0.0.0/8`, loopback `127.0.0.0/8` or `::1/128`, link-local `169.254.0.0/16` or `fe80::/10` (this includes cloud metadata), IPv4 multicast and reserved `224.0.0.0/3`, IPv6 multicast `ff00::/8`, or IPv4-mapped `::ffff:0:0/96`. Skipping the pin trusts the resolver for names that land in the listed ranges: a resolver that rebinds a name to another address inside the same range is not distinguishable from the gateway's placeholder. List only the placeholder ranges the gateway actually returns. A non-empty list changes the webhook binding, so use a separate state directory. An empty or omitted list does not.

The webhook receives a versioned JSON reference with `delivery_id`, Space, Actor, Consumer, item and message IDs, and an identical `Idempotency-Key` header. It receives no body or Waypost credential. A 2xx response means accepted, not processed. A timeout, redirect or any other response after an attempt becomes `unknown` and is not automatically retried. `status --items` lists pending, blocked and unknown item IDs for operator review. `reconcile` without arguments cannot inspect a generic webhook; verify the delivery independently before using `reconcile ITEM_ID accepted` or `reconcile ITEM_ID pending`.

The `scan` command only scans and persists references; it does not notify or acknowledge. A scheduled host may instead call Waypost HTTP or MCP directly. Grok Bot routines are a potential receiver for this generic contract but have not been verified here. Grok.com Automations use a different signed protocol and are not supported by this adapter.

## Bridge-owned OAuth and receiver verification

The bridge can authorize its own public OAuth client without reading Codex, Claude, or another host's MCP credential files. Existing flat and version 2 `token_file` configurations continue to work. To use OAuth, remove `token_file` and add:

```json
{
  "oauth": {
    "credentials_file": "/home/me/.config/waypost-bridge/oauth.json",
    "client_id": null,
    "redirect_port": 0
  },
  "require_receiver_verification": true
}
```

Keep the existing service origin, expected `space_id`, `actor_id`, `consumer_id`, state directory and adapter settings. The credential parent directory must already exist, belong to the current user, and have mode 0700. The JSON configuration remains mode 0600. The credential file is created by `login`; do not create an empty file. Configure exactly one of `token_file` and `oauth`.

```sh
waypost-bridge --config /absolute/path/config.json login
waypost-bridge --config /absolute/path/config.json pair-receiver
waypost-bridge --config /absolute/path/config.json once
waypost-bridge --config /absolute/path/config.json run
```

`login` prints an authorization URL to open in a browser on the same device and waits up to five minutes for a loopback callback on 127.0.0.1. It uses authorization code + S256 PKCE and verifies callback state and issuer. It registers a separate native public client if `client_id` is omitted or null. An administrator may instead register a fixed public client and supply its ID and an allowed loopback `redirect_port`; the server must accept the chosen callback URI. The current message audience is `<base_url>/mcp` and scope is `waypost`. This still grants the current space-member capabilities; a notification-only scope is not implemented.

Before saving any initial or refreshed credentials, the bridge calls `/v0/describe` and checks all three expected IDs. Access and refresh tokens are saved together by a mode 0600 atomic rename, with file and parent-directory fsync. A separate private lock next to the credentials file serializes credential use across state directories. The bridge refreshes before expiry. It first persists `refresh_pending`; if the request or response is interrupted, rejected, malformed, has the wrong identity, or cannot be saved, it leaves that marker and refuses to replay the old refresh token. Run `login` to explicitly recover. It never repeatedly launches login. Keep the credential JSON and its `.lock` file private, and do not replace or delete the lock file while any bridge process is running.

Receiver verification is enabled by default for OAuth configurations. Legacy `token_file` deployments retain their previous behavior; add `"require_receiver_verification": true` to enable the same gate. A legacy deployment without this option checks only the bridge's credential identity and must not be described as having a verified receiver.

`pair-receiver` creates a server challenge bound to a hash of the bridge's pinned service, identity and fixed adapter/thread configuration. It sends one synthetic challenge to that receiver. The receiver must call `waypost_receiver_confirm` with `binding_id` and `challenge` through its own Waypost MCP connection. A webhook integration can use the equivalent authenticated `POST /v0/receiver-bindings/{binding_id}/confirm` with `{"challenge":"..."}`; its own Waypost credential must match the bridge's Space, Actor and Consumer. The receiver must use its own credential, never the webhook Bearer secret. The service refuses a mismatched credential. A Codex queue receipt or a webhook 2xx response means only that the challenge was accepted by the adapter; it does not verify receiver identity.

Before every message dispatch the bridge checks the proof with Waypost and pauses if it is missing, pending, expired, mismatched, revoked, or unavailable. The first attempted dispatch can initiate a challenge automatically. Pending challenges expire after ten minutes; confirmed proof lasts up to 24 hours and remains subject to the service's credential-family and account/member revocation checks. An expired proof starts one renewal challenge and pauses dispatch until confirmation. The private journal records the challenge attempt before sending, so a pending or ambiguous attempt is not automatically resent on each poll or restart. After checking the receiver independently, `pair-receiver --renew` explicitly starts a new challenge. Preserve the state directory and delivery journal across upgrades and restarts.

This proves credential consistency at the time of the handshake. The bridge cannot inspect another application's private MCP credentials or detect every later change to its host configuration. Run `pair-receiver --renew` when the receiver connection changes. Codex notifications also include expected IDs and instruct the receiver to compare them with `waypost_describe` before reading or acting. The receiver must still read and persist the message before submitting `received`, and use the service's claim/fencing protocol before processing; bridge acceptance alone is not a processing receipt.

A webhook receiver that enables verification must handle this additional event before normal `waypost.message.available` events:

```json
{
  "version": 1,
  "event": "waypost.receiver.challenge",
  "delivery_id": "wpwake_v1_...",
  "binding_id": "receiver_...",
  "challenge": "...",
  "target": "wpbridge_v1_..."
}
```

The existing endpoint restrictions, webhook secret separation and request idempotency header apply to this event too. A receiver implementing only the original message-reference contract needs to add challenge confirmation before enabling the gate.
