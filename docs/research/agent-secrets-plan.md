# 密钥产品方案：远程 HTTP MCP + 可选 CLI

日期：2026-10-29。本文是产品决定，事实依据在 [调研](agent-secrets-research.md)。不改现有 keyenv 实现。

## 约束是否成立

三条都成立。

1. **不移植 Infisical，用 Cloudflare 上的存储自己做。** 调研已经说明 Infisical 后端依赖 PostgreSQL、Redis、BullMQ 和 Node.js，Agent Vault 还在专有许可里。个人密钥库用一个 Worker 加 D1 就够。Cloudflare 文档里的 SQL 数据库产品是 [D1](https://developers.cloudflare.com/d1/)；存储产品列表里没有叫 D2 的数据库。免费计划的 D1 额度是每天 500 万行读、10 万行写、总共 5 GB（[Workers 定价](https://developers.cloudflare.com/workers/platform/pricing/)）。个人库的读写远低于这个数。
2. **客户端是远程 HTTP MCP，CLI 可选。** 本机 stdio MCP 依赖本机进程，cloud session 没有那台机器，所以不能当主路径。Cursor 的 HTTP MCP 由后端代发请求，刷新令牌和请求头不进虚拟机（[Cloud Agent capabilities](https://cursor.com/docs/cloud-agent/capabilities)）。Claude Code 的云会话使用用户已经在 claude.ai 授予的 connector（[Claude Code MCP](https://code.claude.com/docs/en/mcp)）。CLI 用来在本机录入密钥，以及在虚拟机里把值注入某一个子进程。Skill 说明如何下载；三家插件也可以带上同一份二进制。
3. **cloud session 不追求进程级保密。** 平台自己的密钥库才能把值挡在模型外面。自建服务一旦把值交给已授权的 agent，模型、工具结果和虚拟机磁盘都可能看到它。接受这一点，用 skill 约束用法。

有一处需要收窄，否则第 3 条会落空：远程 MCP 写不了虚拟机里的文件。如果 `fetch` 把明文放进工具结果，这句话已经在上下文里，事后再「存到本地」挡不住。所以 skill 的顺序是：能用 CLI 注入子进程就不要调用返回明文的工具；必须取回时，写入权限 `0600`、已被 git 忽略的文件，回复里只写路径和字段名。这是纪律，不是沙箱。

## 复用 waypost，不另写网关

上一版把 MCP、认证和 D1 访问写成一个新 Worker。这和 [waypost](https://github.com/miles1150/waypost) `fbfcce6` 里已经在跑的基础设施重复。本仓库只有插件和技能；下面按该提交的服务端源码说。

`src/service/mcp.rs` 的 `App::mcp` 已经是无会话的 HTTP MCP 网关：校验 `Origin`，只接受 `POST`，协商 `2025-11-25`、`2025-06-18`、`2025-03-26`，校验单条 JSON-RPC，`initialize` / `ping` / `tools/list` / `tools/call`，通知返回 202 空体，工具结果同时给 `content` 和 `structuredContent`。`src/service/mod.rs` 的 `handle` 把 `/mcp` 交给它，并在 401 上补 `WWW-Authenticate`。工具目录和 `call_tool` 才是 Waypost 自己的邮箱、记忆业务。

同一套请求还带着 OAuth 资源服务器，设计在 [`docs/design/0010-mcp-oauth.md`](https://github.com/miles1150/waypost/blob/fbfcce6a60b09b9ecc806df507c89d803af2ea73/docs/design/0010-mcp-oauth.md)，实现在 `src/service/oauth.rs`：受保护资源元数据、授权服务器元数据、S256 PKCE、设备码、刷新轮换、撤销。资源固定为 `<origin>/mcp`，scope 目前写死 `waypost`。[`0012`](https://github.com/miles1150/waypost/blob/fbfcce6a60b09b9ecc806df507c89d803af2ea73/docs/design/0012-native-oauth-compatibility.md) 已经用隔离的 Codex CLI 0.163.0 对过这套预注册 public client，包括 loopback 端口变化。官方 MCP SDK 的校验脚本是 `scripts/verify_oauth_sdk.mjs`。

还可以直接用、不必再写的部分：

| 已有代码 | 密钥产品怎么用 |
| --- | --- |
| `src/storage/driver.rs` 的 `Database`，以及 `src/cloudflare.rs` 的 `D1Store` | 查询和 batch 的形状、100 个绑定参数、10 万字节 SQL、超出 JS 安全整数时改用字符串。原生 SQLite 和 Worker D1 走同一个 `App`。 |
| `src/http.rs` 的 `route_label` | 日志里的路径参数换成 `:id`。密钥 id 不能进访问日志。 |
| `src/service/encryption.rs` 的 keyring 配置 | 多密钥、当前密钥 id、轮换。它的密封算法是按 Space 派生的 HPKE，密钥字段继续用 AES-256-GCM，不把 HPKE 封装搬过来。 |
| 幂等 `request_id` 和提交指纹 | 录入和吊销沿用同一套「丢失响应后用原 id 取回原结果」。 |

`token_routing.rs` 是邮箱默认收件人的修改，不是 MCP 网关。邮箱、记忆、邀请、Space/Actor/Consumer 不抽进密钥产品。

抽取放在 waypost 仓库，本仓库只接插件。网关收成一个不认识具体工具的协议层：版本、Origin、JSON-RPC、202、错误码、OAuth 挑战。工具由调用方提供。OAuth 的 scope、资源名和「这个令牌能访问什么」改成参数；现在写死的 `waypost` 和 Space 成员关系留在 Waypost 的适配里。密钥服务实现自己的工具和 D1 表，挂在同一个网关后面。

```text
抽取后的网关（JSON-RPC + OAuth 资源 + D1 适配）
        ├─ Waypost 工具：describe / send / inbox / memory
        └─ 密钥工具：list_entries / fetch_entry
密钥条目仍在自己的 D1 表。数据密钥在 Worker Secret。
```

密钥服务部署在 `keys.miles1150.top`。Cloudflare 能读 Worker Secret，因此这不是零知识库；这是约束 3 接受的信任边界。现有 keyenv 继续负责本机文件库。新产品不走 Google Drive，也不在 Worker 请求里做 Argon2id。字段名、用途、过期和禁用沿用 keyenv 的条目模型。Mac 上用隐藏输入录入，一次导入现有库。

## 存储

D1 四张表。

| 表 | 内容 |
| --- | --- |
| `entries` | `id`、`name`、`purpose`、`disabled`、`expires`、`updated_at`。`name` 唯一。 |
| `fields` | `entry_id`、`name`、`nonce`、`ciphertext`。值用 AES-256-GCM，AAD 是条目 id 加字段名。 |
| `tokens` | 令牌的哈希、标签、`revoked_at`。不存令牌原文。 |
| `audit` | 时间、令牌标签、动作、条目名。不存字段值。 |

Worker 收到明文后加密再写入。吊销就是把 `tokens.revoked_at` 写上，或把条目标成 `disabled`。下一次请求即失效，不依赖客户端缓存。

## HTTP API

CLI 和 MCP 共用同一组路由。MCP 是这组路由的工具包装。

| 方法 | 作用 | 响应里有没有值 |
| --- | --- | --- |
| `GET /v1/entries` | 名称、用途、字段名、是否禁用、是否过期 | 没有 |
| `GET /v1/entries/{name}` | 一个条目的字段值 | 有 |
| `PUT /v1/entries/{name}` | 创建或替换。给 CLI，不放进 agent 的默认工具 | 请求体里有 |
| `POST /v1/run-env` | 按名称返回一组环境变量，给 CLI 注入子进程 | 有，只给 CLI |

认证走抽出来的 Waypost OAuth 资源服务器，不从零写第二套发现端点和刷新轮换。密钥 scope 与 `waypost` 分开，资源是 `https://keys.miles1150.top/mcp`。预注册 public client 沿用 0012 里已经对过 Codex 的做法。传统 Bearer 只留给 CLI 和还不能做 OAuth 的宿主；吊销写在令牌表上，下一次请求失效。

## MCP 工具

远程地址：`https://keys.miles1150.top/mcp`。

| 工具 | 给 agent 的结果 |
| --- | --- |
| `list_entries` | 元数据。查找和确认名称用这个。 |
| `fetch_entry` | 字段明文。只有 CLI 不在，并且下一步必须用这个值时才调用。 |

不提供 `put_entry` 给 agent。录入走 Mac 上的 CLI，避免把新密钥打进对话。

## CLI

一个小二进制，三个子命令。

| 命令 | 谁用 | 行为 |
| --- | --- | --- |
| `keyenv-remote login` | 人，在 Mac 上 | 保存令牌到 `0600` 文件。 |
| `keyenv-remote set` | 人，在 Mac 上 | 隐藏输入，`PUT` 到 Worker。 |
| `keyenv-remote run --entry NAME -- cmd` | agent 或人 | 取值，放进这一个子进程的环境变量，stdout/stderr 原样转发。命令行参数里不放值。 |

Skill 里写明两条获得方式：从固定 URL 下载当前平台的二进制，或使用插件里附带的那一份。下载失败就改用 `fetch_entry`。

## 插件

三份清单指向同一个 URL，附带同一份 skill，可选附带 CLI。协议只有一个，插件只是各平台的包装。

| 平台 | 清单 |
| --- | --- |
| Cursor | `.cursor-plugin/plugin.json`、MCP 配置、`skills/keyenv-remote/SKILL.md` |
| Claude | `.claude-plugin/plugin.json`、`.mcp.json`、同一份 skill |
| Codex | `.codex-plugin/plugin.json`、`.mcp.json`、同一份 skill |

Skill 规定：

- 先 `list_entries`。名称对不上就问，不猜。
- 虚拟机里有 CLI 时，用 `keyenv-remote run` 执行需要密钥的命令。
- 没有 CLI 时，`fetch_entry` 的结果写入 `$TMPDIR/keyenv/<entry>.env`，权限 `0600`，路径在 git 忽略规则里。随后的命令从该文件读。用完删除。
- 回复、提交、日志和后续命令的参数里只出现路径和字段名。
- 禁用或过期的条目不使用。

## 免费额度

一次 `list` 是个位数行读，一次 `fetch` 再加几行。每天 500 万行读、10 万行写，个人使用到不了上限。Worker 免费计划每天 10 万次请求；密钥读取达不到这个量。D1 从 2026-09-01 起对免费计划执行每日限额，超限后查询失败到次日 UTC 零点，已存数据还在（[changelog](https://developers.cloudflare.com/changelog/post/2026-09-01-d1-free-tier-limit-enforcement/)）。

## 分步

**第一版。** 在 waypost 仓库把 MCP 网关、OAuth 资源轮廓和 D1 适配抽成可挂工具的层，并用现有官方 SDK 脚本确认 Waypost 工具行为不变。密钥服务只加条目表、`list_entries` / `fetch_entry` / `set`、CLI `run`、三份插件清单和 skill。从现有 keyenv 做一次导入。

**下一版。** 审计表只记条目名和动作。把还在用静态 Bearer 的宿主迁到同一套 OAuth。

**不做。** 不移植 Infisical。不另写 JSON-RPC、OAuth 发现、刷新轮换或 D1 驱动。不在 Worker 里跑 Argon2id。不把本机 stdio 当 cloud session 的主路径。不把 Waypost 的邮箱和记忆模型并进密钥表。不承诺模型看不到已授权的明文。
