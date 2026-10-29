# 评审：agent 密钥方案（PR #4）

日期：2026-10-29。评审对象是 PR #4（分支 `cursor/agent-secrets-research-7a99`）里的 [方案](https://github.com/miles1150/personal-plugins/blob/cursor/agent-secrets-research-7a99/docs/research/agent-secrets-plan.md) 和 [调研](https://github.com/miles1150/personal-plugins/blob/cursor/agent-secrets-research-7a99/docs/research/agent-secrets-research.md)。代码依据是 waypost `fbfcce6`。外部依据只引用本次实际读到的材料：MCP 规范 `2025-11-25` 授权一节（`modelcontextprotocol/modelcontextprotocol` 仓库 `docs/specification/2025-11-25/basic/authorization.mdx`）、Codex 源码 `openai/codex@a29c7b9`（2026-10-29）、Claude Code 托管的 CIMD 文档 `https://claude.ai/oauth/claude-code-client-metadata`、Claude Code 的 [MCP 文档](https://code.claude.com/docs/en/mcp)。本环境的出口代理拒绝访问 `chatgpt.com`、`developers.openai.com`、`developers.cloudflare.com`、`cursor.com`、`datatracker.ietf.org` 和 `rfc-editor.org`。凡是只出现在这些站点上的说法，下面都标为「未核对」，不当作事实。

结论先说：方向对（Worker + D1 + 复用 waypost 网关与 OAuth，CIMD 为主），但有三处和已定决策冲突，两处低估了 waypost 现有代码的耦合，CIMD 一节有一条事实错误和一个兼容缺口。

## 一、与已定决策冲突（以已定决策为准）

| # | 方案原文 | 已定决策 | 处理 |
|---|---|---|---|
| D1 | 「不提供 `put_entry` 给 agent。录入走 Mac 上的 CLI」；HTTP API 表里 `PUT` 只给 CLI | MCP 必须自己闭环：`list_entries`、`fetch_entry`、`create_entry`、`update_entry`，可选 `disable_entry` | 实现五个工具。写入工具的结果只回成功状态、条目名和字段名，不回显值。读和写分开授权（scope 加同意页按条目勾选）。 |
| D2 | 「CLI 和 MCP 共用同一组路由。MCP 是这组路由的工具包装」，另设 `/v1/entries` REST | 复用 MCP 网关，不另写协议 | 不做 REST。`keyenv-remote` 也调用同一个 `/mcp` 的 `tools/call`，只有一套 API，也就只有一套授权检查。 |
| D3 | 存储表里有自建 `tokens` 表（令牌哈希、标签、`revoked_at`），「吊销就是把 `tokens.revoked_at` 写上」 | 认证复用 waypost OAuth，不另写授权服务器 | 不建自己的令牌表。吊销走 OAuth 授权族（`/oauth/revoke` 或族撤销），下一次请求即失效，这一点与方案的目标一致。 |
| D4 | Skill 让 agent 把结果写到 `$TMPDIR/keyenv/<entry>.env` | 写入 `0600`、已被 git 忽略的临时文件；不复述值；文档写明「这是纪律不是沙箱，录入的值必然经过 agent 上下文」 | 方案只说了「读取」这一半。已定决策下 agent 也会录入，用户在对话里给的新值本身就在上下文里，skill 和威胁模型要明说这一点。 |

## 二、对照 waypost `fbfcce6` 的事实核对

**网关部分基本属实。** `src/service/mcp.rs` 的 `App::mcp` 确实是无会话 HTTP MCP：校验 `Origin`、只收 `POST`、`VERSIONS = ["2025-11-25","2025-06-18","2025-03-26"]`、单条 JSON-RPC、通知回 202 空体、`tools/call` 同时给 `content` 和 `structuredContent`。`src/service/mod.rs` 的 `handle` 在 `/mcp` 上调用 `oauth_challenge` 补 `WWW-Authenticate`。但是 `App::mcp` 把认证写死为 `self.authenticate`（返回带 Space/Actor/Consumer 的 `Principal`），`serverInfo.name` 写死 `waypost`，`instructions` 写死 Waypost 用语。抽取时这三处都要改成由调用方提供。

**「OAuth 的 scope、资源名改成参数」低估了耦合（G1，最重要）。** 不只是字符串 `waypost`：

- `migrations/0014_mcp_oauth.sql` 的 `oauth_families` 把 `space_id`、`actor_id`、`consumer_id` 设成 `NOT NULL`，并有到 `spaces`、`actors`、`consumers` 的外键。不建一个 Space，就写不进一行授权族。
- `oauth_tokens.rs` 的 `LIVE_FAMILY`、`LIVE_REQUEST` 每次签发和刷新都 join `account_spaces`、`actors`、`consumers`，并要求 `c.view='inbox'`。
- 访问令牌写进 Waypost 的 `tokens` 表（同样要求 Space），`mod.rs` 的 `oauth_authority_guard` 用 `account_spaces` 和 `c.view='inbox'` 校验。
- `oauth_authorization.rs` 的同意提交要求会话已选 Space，并必须带 `actor_id`、`consumer_id`。

所以密钥产品不能只换 scope 字符串，必须把「授权绑定」交给产品：Waypost 绑定 Space/Actor/Consumer，密钥产品绑定账号和条目权限。实现上，授权码、PKCE、一次性兑换、刷新轮换、重放撤销、撤销这些状态迁移保持同一份代码，只把绑定相关的 SQL 片段和表交给产品；密钥产品用自己的族表和访问令牌表，Waypost 的表和 SQL 原样不动。这也天然满足「令牌不能跨产品」：不同的表、不同的令牌前缀、不同的 `resource`。

**CIMD/DCR 客户端在认证阶段会被拒（G2，方案未提）。** `oauth_authority_guard` 的条件是 `ofa.client_id IN (SELECT value FROM json_each(?))`，参数是配置里的预注册 client 列表。只在授权端点放行 CIMD/DCR 不够：令牌签出后，第一次 MCP 调用就会 401。白名单要扩展成「预注册 ∪ 启用 CIMD 时的 HTTPS client_id ∪ 未停用的 DCR 记录」。同理，`with_oauth` 要求「1–32 个预注册客户端」，只开 CIMD/DCR 的部署会启动失败。

**`D1Store` 的描述属实。** `src/cloudflare.rs` 的 `prepare` 限制 100 个绑定参数和 100 000 字节 SQL，超过 JS 安全整数的 `i64` 改传字符串；`with_session(Some("first-primary"))`。它目前是 `cloudflare.rs` 的私有类型，还夹着 Waypost 专用的 R2 流式上传。抽取时要把 D1 适配、请求体读取和日志放进网关，R2 留在 Waypost。

**keyring 的描述属实，但不能直接复用密封算法。** `src/service/encryption.rs` 的配置格式是 `{"active": kid, "keys": {kid: 64 位十六进制}}`，最多 4 把，`kid` 走 `check_id`。密封用按 Space 派生的 HPKE。密钥产品沿用这个配置格式和轮换语义，AES-256-GCM 需要新增 `aes-gcm` 依赖（`Cargo.toml` 里没有）。AAD 除了条目 id 和字段名，还应包含账号和 `kid`，防止把一行密文搬到别的账号或别的字段下解出来。

**`route_label` 的描述属实，但保护范围要说清楚。** `src/http.rs` 的 `route_label` 只用于 `cfg(test)` 和 Cloudflare 构建，不在 `ROUTE_WORDS` 里的段都换成 `:id`。它只决定 Worker 自己写的错误日志里的 `route`。Cloudflare 平台侧的 Workers Logs / invocation logs 不经过它：`wrangler.production.example.toml` 打开了 `[observability.logs]`，采样率 1.0。更稳的做法是密钥产品的 URL 路径里根本不出现条目名（所有条目名都在 MCP 请求体里），密钥部署的 wrangler 配置关闭 invocation logs。平台日志里具体记录哪些字段，本次没能打开 Cloudflare 文档核对（未核对）。

**请求体上限会卡住导入（G3，方案未提）。** Worker 的 `Limits.max_json_bytes` 是 16 384。keyenv 的条目模型（`tools/keyenv/internal/vault/model.go`）允许单个值 65 536 字节、最多 64 个字段。密钥产品需要自己的上限，并在服务端单独限制单值和单条目总量。

**`verify_oauth_sdk.mjs` 只覆盖预注册。** 脚本固定 `client_id: 'local-mcp-test'`，依赖 `127.0.0.1:8793` 上的 Workerd 和合成账号会话。它适合确认「抽取后 Waypost 工具行为不变」，不能证明 CIMD 或 DCR，那两条要另写测试。

## 三、CIMD 与登录一节

**属实的部分。** MCP `2025-11-25` 授权规范要求：CIMD 的 `client_id` 是带路径的 HTTPS URL；授权服务器必须核对文档里的 `client_id` 与 URL 完全一致、必须按文档校验 `redirect_uri`、应遵守缓存头、应防 SSRF；客户端优先级是预注册 → CIMD → DCR。Codex 源码 `codex-rs/rmcp-client/src/oauth_client_registration.rs` 证实：元数据里 `client_id_metadata_document_supported` 为 true 且 `token_endpoint_auth_methods_supported` 含 `none` 时，Codex 才走 CIMD；issuer 识别成立时用 `https://chatgpt.com/oauth/codex/client.json` 和 `/callback`，否则用 `https://chatgpt.com/oauth/codex/{callback_id}/client.json` 和 `/callback/{callback_id}`；`callback_id` 是完整 MCP URL 的 SHA-256 前 9 字节（`oauth_callback.rs`）。没有 CIMD 时 Codex 回落到 DCR。

**事实错误：「ChatGPT 的文档还声明 `private_key_jwt`；第一版只接受 `none`，交集里就使用 `none`」（C1）。** 客户端元数据里的 `token_endpoint_auth_method` 是单个值（RFC 7591 §2），不是集合，不存在「交集」。文档声明 `private_key_jwt`，客户端就会在令牌端点用私钥 JWT 认证；只支持 `none` 的服务器应当在授权阶段就以 `invalid_client` 拒绝这份文档，而不是假装能谈出 `none`。ChatGPT 的文档内容本次无法读取（`chatgpt.com` 被出口拒绝，未核对）。第一版的处理：文档里 `token_endpoint_auth_method` 缺省或为 `none` 才接受，否则拒绝，并在文档里写明 ChatGPT 连接要等支持 `private_key_jwt` 后再验收。Codex 不受影响，因为它只在服务器声明 `none` 时才走 CIMD。

**兼容缺口：loopback 规则只写了 `127.0.0.1`（C2）。** Claude Code 的 CIMD 文档（本次实际取回）是：

```json
{"client_id":"https://claude.ai/oauth/claude-code-client-metadata","client_name":"Claude Code","client_uri":"https://claude.ai","redirect_uris":["http://localhost/callback","http://127.0.0.1/callback"],"grant_types":["authorization_code","refresh_token"],"response_types":["code"],"token_endpoint_auth_method":"none"}
```

它登记了不带端口的 `localhost`。Codex 的 CIMD 判断也接受 `localhost`（`native_redirect_supported` 匹配 `127.0.0.1 | localhost`）。RFC 8252 §7.3 的「任意端口」原文针对 loopback IP 字面量；0012 对预注册客户端的 `localhost` 要求固定端口。CIMD/DCR 的规则因此定为：登记的回调是 `http` 且主机为 `127.0.0.1`、`[::1]` 或 `localhost` 时，请求可以带任意非零端口；主机按字面精确匹配（三者互不为别名），路径精确匹配，不允许查询串、片段和用户信息。预注册客户端保持 0012 的现有规则不变。

**风险：localhost 冒充（C3，方案未提）。** MCP 规范「Localhost Redirect URI Risks」一节写明，CIMD 本身挡不住本机进程拿 Claude Code 或 Codex 的 `client_id` 起一个 localhost 监听来骗授权码，同意页会显示合法客户端的名字。规范要求同意页「MUST clearly display the redirect URI hostname」，并「SHOULD」对只有 localhost 回调的客户端加警告。密钥产品的同意页要显示 client_id 所在主机、本次回调主机，并对 loopback 回调加提示。DCR 的 `client_name` 是自报的，要标成「未验证」。

**SSRF 的边界要按运行时写清楚（C4）。** Worker 的 `fetch` 不能先解析 DNS 再连，所以 Worker 上能做的是：只收 `https`、无显式端口、无用户信息/查询/片段/点段、主机必须是带点的域名（拒绝 IP 字面量、`localhost`、`.local`、`.internal` 等）、不跟随跳转、限制响应大小和超时。原生运行时可以先解析、拒绝私有地址、再固定到解析结果连接。两者能力不同，文档要分别写，不能统称「拒绝内网地址」。

**`keyenv-remote` 的 CIMD 文档托管在同一个 Worker 上（C5）。** Worker 去 `fetch` 自己的自定义域名会怎样路由，本次没能核对 Cloudflare 文档（未核对）。更稳的做法是服务端认出「自己托管的那份文档」，直接用内置定义，不发子请求；公开地址上返回同一份 JSON 给其他人核对。

**DCR 的滥用面（C6）。** `/oauth/register` 是匿名写 D1。现有边缘限流会覆盖它（`cloudflare.rs` 对 `/oauth/` 下除 `/oauth/token` 外的 POST 用 `ADMIN_RATE_LIMITER`，生产配置每分钟 10 次），另需要一个总量上限，并清理从未完成授权的注册。方案里「同一连接复用已发的 `client_id`」是客户端行为，服务端保证不了。

## 四、其他问题

- **Skill 写文件这一步本身会再暴露一次值（S1）。** 远程 MCP 写不到虚拟机的磁盘。agent 要写文件，只能把工具结果里的值再写进一条 shell 命令或文件写入调用，这些调用也会进 transcript。Skill 只能要求「用一次写入、不 echo、不放进命令行参数」，这同样是纪律。威胁模型里要写明。
- **`.env` 格式的引号问题（S2）。** keyenv 自己的 `Export`（`internal/app/export.go`）对 dotenv 拒绝含 `$`、反斜杠、单引号、换行的值。skill 让 agent 手写 `.env` 会在这些值上出错或被 shell 展开。建议默认写 JSON，由后续命令自己解析；需要环境变量时用 `set -a; . file` 前先确认值里没有这些字符。
- **「`$TMPDIR` 在 git 忽略规则里」不成立（S3）。** `$TMPDIR` 通常不在仓库里，谈不上被忽略；写在仓库内时才需要 `.gitignore`。skill 应写成：优先写到仓库外的私有临时目录；必须在仓库内时，先确认路径被 `git check-ignore` 命中。
- **条目列举的最小权限（S4）。** 方案的 `list_entries` 返回全部元数据。按条目授权以后，未授权的条目名也不应出现在列表里，否则名称和用途本身就泄露了用户有哪些账号。实现成只列出本次授权能读或能写的条目，并标出 `readable` / `writable`。
- **字段的 `secret` 标记（S5）。** keyenv 的 `Field` 有 `Secret bool`。远程库一律加密存储，不区分，列表只给字段名；导入时丢弃这个标记，文档说明。
- **未核对的额度数字（S6）。** D1「每天 500 万行读、10 万行写」、Workers「每天 10 万次请求」、2026-09-01 起免费层超限即失败，都只出现在 Cloudflare 文档里，本次无法打开，保留原文但标为未核对。Rate Limiting 绑定在免费计划上是否可用同样未核对；生产配置要求这两个绑定，缺了会拒绝服务（`cloudflare.rs` 的 `production && (admin_limiter.is_err() || api_limiter.is_err())`）。
- **插件清单的位置（S7）。** 方案写 Codex 用 `.codex-plugin/plugin.json`。Codex 源码 `core-plugins/src/loader.rs` 既读 `.codex-plugin/plugin.json`，也读 Agent Plugins 格式（本仓库现有插件用的根目录 `plugin.json`），MCP 默认读插件根的 `.mcp.json`。本仓库沿用现有约定：根目录 `plugin.json`（Codex）、`.claude-plugin/plugin.json`、`.cursor-plugin/plugin.json`，三者共用一份 `.mcp.json` 和一份 skill。Cursor 的清单格式本次无法打开文档核对（未核对）。
- **Cursor 与 Claude 云会话「复用宿主保存的授权」（S8）。** 调研对 Cursor 后端代发、刷新令牌不进虚拟机的描述来自 `cursor.com`，本次未核对。Claude Code 文档写明支持 CIMD 并会自动发现，没有 CIMD 时走 DCR，这一点已核对；云会话使用 claude.ai connector 的具体机制本次没有读到原文。

## 五、据此调整后的实现顺序

1. waypost：把 MCP 协议层抽成不认识工具的网关（工具目录、认证、`serverInfo`、`instructions` 由调用方注入）；授权状态机保留一份，绑定交给产品（G1）；客户端解析扩展成「预注册 → CIMD → DCR」，同时修正认证白名单和启动校验（G2）；元数据声明 `client_id_metadata_document_supported: true`、`registration_endpoint`，令牌端点仍只收 `none` 加 PKCE（C1）；CIMD/DCR 的 loopback 规则按 C2；同意页显示主机和警告（C3）；SSRF 规则按运行时分别实现（C4）。
2. 密钥产品作为网关的第二个调用方：自己的条目表、族表和访问令牌表，AES-256-GCM（AAD 含账号、条目、字段、`kid`），五个工具，读写分开授权，独立的请求体上限（G3），自托管 `keyenv-remote` 的 CIMD 文档在服务端内置解析（C5）。
3. `keyenv-remote` 调用同一个 `/mcp`（D2）；skill 写明纪律边界（D4、S1–S3）。
