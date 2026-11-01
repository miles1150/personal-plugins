# Codync Network and Miles Works

Architectural comparison only. This note does not change either product.

Read on 2026-11-01.

| Source | What was read |
|---|---|
| Launch post | [x.com/lbki34064963/status/2116543290174832810](https://x.com/lbki34064963/status/2116543290174832810) (2026-10-31): Codync Network (beta) as "Agent Tailscale", agents addressed `agent@computer.network`, no ports and no VPN |
| Codync | [leepokai/Codync](https://github.com/leepokai/Codync) `355a865459557a679ba0329ab0924caa8e8342b7` (2026-11-01, merge of PR 286). The network layer is vendored at `host/crates/network` (crate `codync-network` 0.2.1). The README says the crates.io package is a read-only mirror; pull requests go to this path. No separate crate checkout |
| Crate docs | Module docs in that tree match [docs.rs/codync-network](https://docs.rs/codync-network/latest/codync_network/) |
| Product site | [codync.dev](https://www.codync.dev) on the same day. Its Tailscale and ciphertext-relay copy describes the phone-to-computer channel. The agent mesh is specified in the repo, not on that page |
| Miles Works | This repo `1b30c263458da4c04f9a716a95aa6145541c250e`. Plugin `plugins/miles-works` 0.7.2. Service `services/waypost`. Deployed origin in `services/waypost/docs/unified-deployment.md`: `https://works.miles1150.top`, Worker `miles-works-production` |
| Waypost history | `plugins/miles-works/UPSTREAM.md`: service imported from `miles1150/waypost` `11c4668f63ff0ef2c67026ff12c061d6c7ac980f`. The comparison uses the code in this repo |

Line numbers below refer to those trees.

## What each one is

Codync Network is a private mesh of the user's computers. Each computer runs a `Node` with an iroh endpoint key. Agents on those computers exchange short letters. Membership is a signed roster every member stores. A relay, when one is used, forwards QUIC ciphertext and does not store mail.

Miles Works 0.7 is a personal control plane on one Cloudflare Worker. A person signs in with Google, approves an OAuth client, and mounts that client on an assistant. Assistants in that account exchange messages that the Worker stores, assigns, and later decrypts for an authorized reader. An optional bridge on another process can wake a Codex thread or a webhook; the execution client still reads and claims through MCP.

The shared sentence is "agents on different computers message each other." The jobs underneath that sentence are different.

## Transport

**Codync.** Computers dial each other with iroh over QUIC. The application protocol is one JSON request and one JSON response per bidirectional stream, ALPN `codync-network/0` (`host/crates/network/src/wire.rs:14`, `wire.rs:154-159`). `Connector` picks the fallback (`types.rs:12-22`, `node/endpoint.rs:54-63`):

| Connector | Path |
|---|---|
| `public` (default) | Direct QUIC, then iroh's public relays (n0, rate-limited, described as for trying it out) |
| `https://…` | Direct, then the user's own `iroh-relay` |
| `private` | Direct only: same network, Tailscale, or a VPN. Relay mode disabled |

Hole punching and moving from relay to a direct path are iroh's. When addresses change, the computer signs a new `Locate` into the roster and says hello (`node/endpoint.rs:35-51`, `roster.rs:94-97`). There is no Codync server on this path. `docs/design/codync-network.md` (around the "How two computers reach each other" section) says Codync's Cloudflare relay is a different channel: phone to computer, and cloud assistants. `relay/` is an APNs ticket worker. `private-relay/` is a WebSocket room relay for that phone channel and says it does not decrypt chat. Neither is the agent mesh.

**Miles Works.** Every `works_*` call is HTTPS MCP to one origin, `https://works.miles1150.top/mcp` (`services/waypost/docs/unified-deployment.md`). State is D1 (`DB`, and a separate `KEYS_DB`) plus R2. Clients do not dial each other. A host that is off is simply absent; the Worker still holds the delivery row. There is no QUIC, no hole punching, and no per-computer address advertisement.

A Codync letter moves only while the sender can open a QUIC connection, or while that sender process still has the letter in memory. A Miles message moves when any authorized client of the recipient later calls the Worker.

## Identity and authorization

**Codync.** A computer's identity is its endpoint public key (`Node::id`, `node/mod.rs:117-118`). The key file is `secret.key`, mode `0600` on Unix (`node/endpoint.rs:15`, `node/endpoint.rs:77-90`). The network id is a separate root public key. `Roster::create` returns that root and the caller is told to show it once and keep it nowhere (`roster.rs:151-161`, `node/mod.rs:135-146`). The recovery code can later sign `Change::Owner` and move ownership (`node/mod.rs:214-228`).

Joining requires a one-time invite. `Node::invite` stores a random secret in memory for one hour and only while that process stays up (`node/mod.rs:27`, `node/mod.rs:149-158`). The ticket carries the network id, the secret, the inviter's `EndpointAddr`, and an optional network name (`wire.rs:118-127`). `admit` checks the secret once, refuses a key that was removed, and signs `Change::Admit` (`node/inbound.rs:163-188`). A removed computer stays removed until it joins again with a new key (`roster.rs:12`).

Roles are owner, admin, and member (`roster.rs:38-62`). The owner invites, removes anyone, and promotes. Admins invite and remove plain members. Copies of the log merge; each entry is an ed25519 signature over network, signer, time, and change (`roster.rs:106-130`, `roster.rs:217`). The design note says the cloud may shorten access and may not decide membership.

Who may write a letter is a separate list on the receiving computer. Rules are first-match, default allow when the list is empty (`rules.rs:1-3`, `rules.rs:103-105`). The daemon persists them in `rules.json` and loads them into the node (`src/bin/codync-network/main.rs:35`, `main.rs:99`, `main.rs:208-211`). An answer to a request this computer sent still passes, so a rule added mid-flight does not strand the reply (`node/mod.rs:381-386`, `node/inbound.rs:100-107`).

There is no user account on the mesh. Sign-in for Codync as a product is a different, optional path (Clerk, for cloud assistants).

**Miles Works.** The live principal is an account, an assistant, an installation, and an OAuth family (`services/waypost/src/service/works_v2.rs:1-16`, `works_v2.rs:35-63`). The account is a verified Google identity. `works:client` is the scope a new client gets: it identifies the installation and allows a draft or a mount request. It does not grant messages or keys (`services/waypost/docs/unified-deployment.md`, "New OAuth clients default to `works:client`"). A missing scope normalizes to that value (`oauth.rs:195-199`). The v2 token endpoint rejects any other scope (`oauth_tokens.rs:380-381`) and issues `scope` `works:client` (`oauth_tokens.rs:493`). The metadata document still lists older scope names beside it (`oauth.rs:186-193`); those names are not the v2 message grant.

Clients register dynamically when discovery enables it (`POST /oauth/register`, stored `source='dcr'`, `oauth_client_registry.rs:374-396`) and the authorization-server metadata can also advertise Client ID Metadata Documents (`oauth.rs:306-322`). The owner approves the client in the browser. Mounting is a second explicit approval. Every mounted client inherits that assistant's full policy (`works_v2.rs` guard requires `messages_enabled`, an active installation, an active attachment, a live family, and the policy revision).

`works_delegation_check` is another grant, stored apart from mounts and from message text. It requires the current assignment, a verified sender session, and a positive action mask: analyze, code, create task, coordinate, reply (`works_sessions.rs:62-88`). The plugin skill says a message cannot create or enlarge that grant (`plugins/miles-works/skills/waypost/SKILL.md`).

The Space / Actor / Consumer model is the retired bridge runtime. Plugin notes for 0.7.1 say that runtime is retired and that old bridge configs are rejected before the journal opens (`plugins/miles-works/RELEASE-NOTES.md`). The unified Worker returns `LEGACY_PROTOCOL_RETIRED` (HTTP 410) for public `/v0` and for a token that still resolves only as a legacy parent (`src/service/mod.rs:242-248`, `src/service/unified.rs:49-60`). v2 bootstrap does not carry an actor. `services/waypost/docs/security.md` still describes a Space as the data boundary and an operator who can decrypt. That page lags the 0.7 principal. The message path that actually runs seals with the account id as the HPKE associated data (`encryption.rs:154-162`, `works_sessions.rs:390`).

## Addressing

**Codync.** The crate address is `agent@computer` (`types.rs:60-84`). A bare agent name resolves on this computer first, then to the single other computer that offers it; two matches are an error that names both (`node/directory.rs:96-121`). A taken computer name becomes `name-2` (`roster.rs:207-214`). Computer names reject space, `/`, `@`, and `.` so they cannot break the address (`roster.rs:35-36`).

The host, not the crate, adds the network when this computer is in more than one: `agent@computer.network` (`host/src/network/mod.rs:161-165`, `mod.rs:198-208`). Each network is a separate node, key, roster, connector, and rule set, so one network's members do not see another network's key (`docs/features/codync-network.md`, "Several networks"). Bot address names are not reused (`network_handles` in that feature note): a letter in flight cannot land on a different bot that inherited the name.

`set_senders` lets the app send as a name it does not offer, such as a hosted assistant that can ask and cannot be asked (`node/mod.rs:251-255`).

**Miles Works.** Routes are opaque ids inside one account. `works_messages_send` takes `recipient_assistant_id`, optional `session_id`, `conversation_id`, and `idempotency_key` (`works_mcp.rs:77-88`). `works_recipients_list` returns assistants in the caller's account that have messages enabled, plus discoverable registered sessions (`works_sessions.rs:153-164`). Display names are labels. They are not lookup keys. There is no `agent@computer` form, no cross-account directory, and no computer name in the route. A session is a registered host conversation on an installation (`works_sessions.rs:90-125`), with `host_kind`, an external session id, and capability `pull` or `wakeable`.

## Delivery reliability

**Codync.** A `Letter` becomes an `Envelope` on the wire: id, from-agent, to-agent, body, `wake`, `reply_to`, `hops`, `meta`, `sent_at` (`types.rs:86-147`, `wire.rs:54-72`). The sending computer is whoever authenticated the QUIC connection; it is not a field. Limits: body 64 KiB, meta 16 KiB, id at most 64 bytes (`wire.rs:15-21`, `wire.rs:194-208`). `meta` is application structure the network does not interpret (`lib.rs:10-13`).

`send` returns `Delivered` on `Stored`, or `Queued` when the peer is away or answers `Later` (`types.rs:183-188`, `node/mod.rs:312-363`). `Later` means the peer has not called `set_agents` since start (`node/inbound.rs:88-91`). The outbox is a `Vec` in the node process: at most 200 letters, kept 24 hours, retry every 30 seconds by saying hello, roster resync every 6 hours (`node/outbox.rs:10-14`, `node/outbox.rs:22-30`, `node/outbox.rs:65-87`). It is not written to disk. A process restart drops it. A refused waiting letter is a log line; the comment in `flush` says a delivery-report channel is future work (`node/outbox.rs:46-50`).

Duplicate suppression is also memory: the last 1000 `peer:letter-id` keys answer `Stored` without handing the letter over again (`node/inbound.rs:19-20`, `node/inbound.rs:92-96`). After restart, or after that window slides, a retry can be delivered again.

The receiver also enforces budgets, ignoring whatever the sender's agent was told (`guard.rs:1-15`, `guard.rs:45-68`):

| Limit | Value |
|---|---|
| Handoff chain | 8 (`MAX_HOPS`) |
| Per pair of agents | Burst of 5, then one per 2 minutes |
| Identical body, meta, and `reply_to` | Refused for 10 minutes |

`Stored` means the app inbox accepted the message or the harness knock returned success. It does not mean the agent finished the work. The Codync host, above the crate, sends a later letter with `meta.codync` of `completed`, `failed`, or `cancelled` (`docs/features/codync-network.md`, "Bots across computers"). The crate itself has no claim, lease, or completion state.

**Miles Works.** Send inserts `works_messages` and `works_deliveries` in one batch. The body is sealed before insert. The idempotency key is unique per account and sending assistant; a reuse with a different fingerprint is `IDEMPOTENCY_CONFLICT` (`works_sessions.rs:375-520`). Routing states include `assigned`, `waiting_for_session` when nothing is targeted and there is no default, and `needs_handoff` when the conversation already has another live owner (`works_sessions.rs:449-452`). A successful send is a stored row. The skill says that is not processing.

The execution client reads the plaintext only after authorization, then claims. Claim is allowed from `assigned` only, for the assigned session on this installation, at the expected `assignment_generation`. It increments `claim_generation` and the conversation generation and sets a lease of 1 to 300 seconds, default 60 (`works_sessions.rs:584-643`). Renew and complete must present assignment generation, claim generation, conversation generation, and an unexpired lease (`works_sessions.rs:645-711`). Outcomes are `completed`, `failed`, or `unknown`. An expired claimed lease is moved to `unknown` on read, and the comment says that is evidence of an unknown outcome, not permission to run the work again (`works_sessions.rs:522-536`). External side effects are still the client's problem: the skill says the claim fences server completion and does not promise exactly-once execution.

So Codync deduplicates a transport retry inside one process. Miles fences which client may finish a durable assignment, and records when that fence was lost.

## Wake and pairing

**Codync.** `Reach` says how a letter enters an agent (`reach.rs:22-33`):

| Reach | Behavior |
|---|---|
| `App` | Push the `Message` on an in-process channel. The app delivers it. Inbox cap 256; overflow refuses (`node/mod.rs:31`, `node/inbound.rs:146-148`) |
| `ClaudeCode { session }` | Find the session's inbox socket under `~/.claude/sessions` (or `CLAUDE_CONFIG_DIR`) and write a JSON `user` frame. Unix only; Windows returns an error (`reach.rs:138-164`) |
| `Codex { thread }` | Run `codex queue --thread <id> --message <text>` (`reach.rs:166-178`) |

A notice (`wake: false`) to Claude or Codex is held, at most 20, and flushed with the next waking letter (`reach.rs:59-86`). The text tells the session the words came from another computer's agent and cannot approve anything for the user, and it prints the reply command (`reach.rs:88-103`). Offering an agent is `codync-network agent <id> claude <session>` or `codex <thread>`. There is no separate pairing secret: the local daemon is allowed to knock because it is on that computer. A Claude session that skips permission prompts holds outside messages unless `crossSessionInbound` is `accept`; the daemon attaches the messaging token only when it is posting to its own session socket (`reach.rs:145-150`, crate README).

Presence is not pushed. `online` means the last hello got an answer (`node/directory.rs:80-88`). Asking `peers` is what creates traffic.

**Miles Works.** Wake is a second installation. `works_wake_create` runs on the bridge's family and inserts a binding only when the bridge installation and the execution installation are both mounted, distinct, and the session is `wakeable` (`works_wake.rs:110-142`). The response includes an HMAC challenge over the binding fields, keyed by the server CSRF key (`works_wake.rs:12-32`, `works_wake.rs:165-169`). The challenge is good for 10 minutes (`CHALLENGE_MS`). `works_wake_confirm` must be called by the execution installation, with that challenge (`works_wake.rs:171-229`). A verified binding lasts 24 hours (`PROOF_MS`) and proves pairing, not that a message was executed (plugin skill).

`works_wake_poll` and `works_wake_check` return delivery id, message id, session, and assignment generation. The select list has no ciphertext (`works_wake.rs:308-311`). The webhook contract posts `miles-works.message.available` with those ids and no body; the receiver reads and claims on its own MCP connection (`plugins/miles-works/skills/waypost/references/setup.md`). HTTP 2xx means the endpoint accepted the notice. The bridge journal treats a timeout or non-2xx as `unknown` and does not resend it by itself.

Direct MCP needs no bridge. 0.7.0 notes say the wake adapter supports Codex, and Claude uses direct MCP. The webhook path is a reference contract; the same note says Grok Bot end-to-end behavior has not been verified.

## Multi-host bridge to coding agents

**Codync, two shapes.**

1. The standalone daemon on each computer offers Claude and Codex sessions and knocks them locally. An agent on computer A addresses `reviewer@desk` and the desk daemon writes into that session.
2. The Codync host offers its own bots through `Reach::App`, and can expose `list_bots`, `message_bot`, and `read_replies` to a local Claude or Codex via `codync-host mcp network` on loopback (`docs/features/codync-network.md`). Cloud assistants are a third path: MCP at `mcp.codync.dev`, through the computer's relay socket, and the design note says they are not end-to-end encrypted because the cloud has to speak MCP (`docs/design/codync-network.md`, "Cloud assistants" and "Who sees what").

**Miles Works.** The coding agent is an MCP client of the Worker. The plugin's `scripts/bridge.py` is a launcher: it checks a bundled `waypost-bridge` digest and execs it (`plugins/miles-works/skills/waypost/scripts/bridge.py`). The source plugin does not contain that binary. The bridge has its own OAuth installation and its own mount, which carries the assistant's full permissions. Pairing asks the Codex session, using that session's own mounted MCP, to confirm the challenge. The webhook variant does the same for any receiver that implements the v2 event. One account can have many installations; they share the assistant by mount, not by being on one QUIC mesh.

Codync injects letter text into the harness. Miles notifies with ids, and the harness pulls the body with its own credential.

## Threat model

**Codync computer-to-computer.** iroh authenticates both endpoints and encrypts the QUIC session. A relay that is only forwarding sees ciphertext (`lib.rs:6-9`, `docs/design/codync-network.md` table "Who sees what": "the two computers only; a relay sees ciphertext"). The letter body is ordinary JSON inside that session. Both computers read it. There is no extra seal from agent to agent. A person who can read the daemon's state or the receiving session can read the letter. Rules and budgets are enforced by the receiver, so a modified sender cannot skip them, but a member computer that is allowed to write can send anything the rules allow. Invites and the recovery code are bearer secrets. The invite exists only in the inviter's memory; the recovery code is the root private key.

The public iroh relays are operated by n0 and are documented as rate-limited and not the production plan. The design's next step is Codync-run `iroh-relay` processes. Those still would not be a mailbox.

**Codync cloud assistant and phone relay.** Different threat model, stated in the design doc: the cloud and the assistant provider can read cloud-assistant tool calls; the user opts in on the computer. The phone channel is a separate end-to-end design (`docs/reference/remote-relay.md`) and is not the agent mesh.

**Miles Works.** The client sends the body in the MCP request. The Worker seals it with HPKE (X25519, HKDF-SHA256, ChaCha20-Poly1305) under the deployment keyring, with the account id and `works-message:{id}` bound into the context (`encryption.rs:1-12`, `encryption.rs:154-162`, `works_sessions.rs:357-390`). Send fails closed if encryption is not configured. Authorized `works_messages_read` and the account dashboard both open that field and return plaintext (`works_sessions.rs:549-579`, `works_sessions.rs:785-797`). Anyone who holds `WAYPOST_ENCRYPTION_KEYS` and the database can read historical message bodies. `security.md` says this explicitly for the storage design: the server sees plaintext, then stores ciphertext; a compromised runtime is not in the threat model the encryption claims to stop. HTTPS protects the hop from client to edge. Relations, sizes, and ids are queryable as normal columns.

The wake webhook can be loopback HTTP if the operator sets `local_loopback`. The setup guide says that mode can expose the webhook bearer and the reference metadata to another local process that binds the port. The webhook body still has no message content and is not an OAuth token. OAuth tokens and refresh tokens stay on the installations that minted them; the skill forbids copying a host token into the bridge.

Default allow also differs. An empty Codync rule list lets every member write. A Miles mount does not authorize autonomous work; delegation is opt-in per receiving attachment and sending assistant.

## Operator experience

**Codync.** On each computer: `init` (prints the recovery code once), `invite`, `join`, then `agent` to attach a session. The desktop app can add an SSH-managed computer by minting an invite locally and joining through the tunnel, so the user does not paste the ticket (`docs/features/codync-network.md`, "Set it up"). Membership APIs are loopback-only. There is no browser consent and no per-message permission. The cost of that simplicity is the recovery code, a process that must stay up to invite or to drain the outbox, and rules that are easy to leave at "everyone in the network."

**Miles Works.** Install the plugin, complete browser login, review the client and `works:client`, then approve a mount. The assistant cannot approve itself. The optional bridge is a second login and a second mount, then a pairing challenge the execution session confirms. Dashboard copy is where delegation is granted and revoked. That is a lot of steps next to one invite. Those steps are the product: the user sees the client, the assistant policy, and the delegation flags before any autonomous work.

0.7.2's unreleased note relaxes a 15-minute local-session-age gate for bootstrap and mount approval. It does not remove the explicit approval, and it does not add step-up authentication (`RELEASE-NOTES.md`).

## What each one is not

**Codync Network is not:**

- A mailbox that survives the sender restarting. The day-long wait is process memory.
- A completion protocol. `Stored` is acceptance by the inbox or the harness.
- Agent-to-agent encryption. The computers are the ends.
- An account system. Account networks are listed as future work in the design note.
- The phone relay or the cloud-assistant MCP path. Those are server-mediated and, for cloud assistants, server-readable.
- A way to run on Windows into Claude Code. The crate says that knock is unsupported.
- Production-default relays. `public` is the default and is documented as a trial path.

**Miles Works 0.7 is not:**

- A NAT-traversal mesh or a computer identity. Installations do not discover each other by public key.
- End-to-end between agents. Storage encryption assumes the Worker sees plaintext.
- A cross-user agent directory. Recipients are assistants in the same account.
- A guarantee that a webhook 2xx means the Codex or Claude session did the work.
- A verified Grok Bot adapter. The setup guide says that compatibility is not established.
- The old Space / Actor / Consumer bridge. That runtime is retired on the unified Worker; `import-legacy` quarantines references and does not replay the old journal.
- A key vault and a messenger glued into one permission. Keys use the same account and a separate database and grant; this note does not treat them as part of the letter path.

## Verdict

**English.** They are complementary. They are not two implementations of one problem.

Codync Network answers: how do agents on the user's own computers find each other and hand over a letter when there is no membership server, no open port, and no account? The trust boundary is the computer key and the signed roster. Delivery is best-effort while both processes are up, with a short in-memory delay if the receiver is briefly gone.

Miles Works 0.7 answers: for assistants the user has deliberately mounted, how is a piece of work stored, assigned, claimed, renewed, and completed, including when the receiving host is off, and what may that receiver do without asking again? The trust boundary is the Google account, the OAuth installation, the assistant policy, and a separate delegation. Delivery is a durable row the operator's Worker can read.

They meet where a local coding agent must be told that work arrived. Codync pushes the letter into Claude's socket or Codex's queue. Miles publishes a reference and requires the execution client to pull, claim, and complete. Using one as the blueprint for the other would drop the property each one actually has: Codync's lack of a coordinating server, or Miles's durable fence and explicit permission.

**中文。** 两者是互补关系，不是同一个问题的两种实现。

Codync Network 回答的是：用户自己的几台电脑上的 agent，在没有成员服务器、不开放端口、也不需要账号的情况下，怎样互相找到并交一封信。信任边界是电脑的端点公钥和各成员保存的签名花名册。两边进程都在时，投递是尽力而为；对方短暂不在，发件进程的内存里可以再等一段时间，进程一退，等待中的信就没了。

Miles Works 0.7 回答的是：用户明确挂载过的助手之间，一件工作怎样被存下、指派、认领、续租并完成，接收方关机之后记录还在，以及接收方在不再询问用户的情况下可以做哪些动作。信任边界是 Google 账号、OAuth 安装、助手策略，以及单独保存的委托。投递是控制面里的一行持久记录，运行 Worker 的人能够读到正文。

它们只在「要告诉本机上的编码 agent 有工作到来」这一点相遇。Codync 把信写进 Claude 的套接字或 Codex 的队列。Miles 只发布引用，由执行端自己拉取、认领并完成。拿其中一个当另一个的蓝图，丢掉的正好是各自真正成立的性质：Codync 没有协调服务器，Miles 有持久围栏和显式授权。

## Takeaways for Miles Works

These are design observations. They are not a port of Codync and not a change list for the Worker.

- Keep the Worker as the record of assignment, lease, generation, and delegation. Codync's outbox is the wrong durability model to copy: it is a `Vec` in the sender, capped, and gone on restart. The useful contrast is already implemented: Miles still has the row when the sender process is gone, and an expired lease becomes `unknown` instead of a silent second run.
- A direct path, if one is ever added between two installations, should still finish through the same claim and generation checks. Codync's `Stored` only means the harness accepted bytes. Miles already separates accept, claim, and complete; a new transport should not collapse those.
- Leave routing on opaque assistant and session ids. A display form such as a session label at an installation label can be shown to people. Codync had to freeze bot address names forever so an in-flight letter could not hit a recycled name. Miles avoids that class of bug by not routing on names.
- Keep mount and delegation apart. Codync's empty rule list allows every member of the network to write. That default matches a mesh of one person's computers. It does not match a client that has the assistant's full policy as soon as it is mounted. The action mask on `works_delegation_check` is the stricter, and appropriate, analogue of Codync's receiver-side rules.
- A receiver-enforced chain budget is the piece Codync has that the Miles message protocol does not. Eight hops, a burst, and a repeat window are checked on the receiving computer no matter what the sender claims (`guard.rs`). Delegated replies can already chain: a receiver with `reply` may message the verified sender, which may delegate back. A budget stored with the conversation, checked when a send is accepted, would limit that loop without adopting a mesh. Message text should not be what carries the hop count, because the sender can lie; Codync's receiver re-checks `hops` for that reason.
- Keep wake as a reference, confirmed by the execution installation. Codync's knock writes a `user`-role frame into Claude and shells out to `codex queue` with the letter body. That is convenient and it is a wider local trust assumption: the daemon on the box may speak as the user into that session. Miles's split (bridge credential ≠ execution credential, webhook has ids only, confirm is an HMAC the execution client must present) is the property to protect.
- Say the threat model the way Codync's design note separates its paths. Computer-to-computer Codync is ciphertext at the relay and plaintext at both computers. Cloud-assistant Codync is server-readable and labeled as such. Miles message storage is the second kind: the Worker seals what it has already seen, and the dashboard opens it for the account. `security.md` still explains that in Space language while 0.7 authorizes by account, assistant, and installation. Updating that description would stop a reader from treating storage encryption as end-to-end mail.
- Do not import invite-as-membership, recovery-code root keys, or public relay defaults. Account recovery is already the Google identity plus family revocation. A one-hour bearer invite is a good join gesture for machines that have no user account. It skips the client review Miles exists to force.
- Treat Codync's cloud-assistant shape as the nearer cousin when comparing products out loud: MCP, an account, a computer that must opt in, server-readable calls, and `read_replies` because a hosted assistant cannot be woken. Miles already has a local wake for a mounted execution session. The gap that remains is a hosted assistant with no process on the user's machine, which neither the mesh nor the webhook covers by itself.
