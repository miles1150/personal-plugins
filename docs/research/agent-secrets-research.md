# Agent 在临时 cloud session 里使用密钥

调研日期：2026-10-29。本文只记录已核对的文档和源码，不改 keyenv 实现。按这些事实做的产品决定见 [方案](agent-secrets-plan.md)：agent 只用远程 HTTP MCP，录入用本机 CLI，存储用 D1，MCP 网关复用 waypost；登录以 CIMD 为主、DCR 兼容、预注册兜底，安全约束放在 skill 里。

读者是这个仓库的维护者：本机已经有 keyenv，Cloudflare 账号在免费计划上、域名是 `miles1150.top`，没有确定可用的 VPS，只打算用免费额度。目标是让 Cursor、Codex、Claude Code 这类每次都换一台新机器的 cloud session 在没有人 unlock、也没有人重做 OAuth 的情况下用到密钥，并尽量不让明文进入模型上下文和日志。

## 结论

1. 不要把 Infisical 后端移植到 Cloudflare Workers 或 D1。2026-10-29 的后端提交 [`f943528`](https://github.com/Infisical/infisical/commit/f943528f8f51d469fa6ef4ccb62d84d14b291cb8) 把 PostgreSQL 和 Redis 都写成启动条件，队列用 BullMQ，进程是 Fastify，引擎要求 Node.js 26。数据迁移有 603 个文件。Agent Vault 的实现在专有许可的 `ee/` 里。这是重写，不是换数据库驱动。
2. 能让 cloud session 不预置引导密钥的，是平台自己保存登录态、再由平台代发请求的路径。Cursor 的 HTTP MCP 把刷新令牌和请求头留在后端，工具调用由后端代理，虚拟机里没有这些凭据（[Cloud Agent capabilities](https://cursor.com/docs/cloud-agent/capabilities)）。Claude Code 的云会话对 claude.ai connector 不在会话里做登录，会话代理使用用户在 claude.ai 授予的授权（[Claude Code MCP](https://code.claude.com/docs/en/mcp)）。1Password 的 Environments MCP 是本机桌面应用的 stdio 服务，文档写明目前不支持只支持远程传输的客户端（[1Password Environments MCP](https://www.1password.dev/environments/mcp-server)），所以它解决不了没人坐在电脑前的 cloud session。
3. 在免费额度内，先用各平台已经提供的 secret / network secret / connector。需要一个统一吊销点时，再在 Workers 上写一个很小的凭据代理，复用 keyenv 的 AES-GCM 封装和“只给元数据、向子进程注入”的操作约束。主密码的 Argon2id 留在 Mac 上，不要放到免费 Worker 的请求路径里。

## 调研范围和材料

对照了本仓库的 [keyenv 设计](../keyenv-design.md)、[CLI 说明](../../tools/keyenv/README.md) 和 [skill](../../plugins/keyenv/skills/keyenv/SKILL.md)，以及 `tools/keyenv/internal/vault/crypto.go`。

上游源码按下面这些提交阅读，不靠架构图推断：

| 材料 | 提交或页面 |
| --- | --- |
| Infisical 后端 | [`f943528`](https://github.com/Infisical/infisical/commit/f943528f8f51d469fa6ef4ccb62d84d14b291cb8)（2026-10-29） |
| Infisical CLI `run` | [`baf672b`](https://github.com/Infisical/cli/commit/baf672b6b4296492d842601de7b79066e3484d72) 的 `packages/cmd/run.go` |
| `@infisical/mcp` | [`d19e3ca`](https://github.com/Infisical/infisical-mcp-server/commit/d19e3ca244d0c1b2506d39803a73e6e2c3730efa) 的 `src/index.ts` |
| Codex MCP OAuth 存储 | [`codex-rs/rmcp-client/src/oauth.rs`](https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs) |
| OpenBao、Vault、SOPS、age、Teller、Phase 许可证 | 各仓库 `LICENSE` 文件，见文末 |

没有读到实现、因此不能下结论的部分集中在文末「没有核对到的边界」。

## 1. keyenv 现在卡在哪里

keyenv 是本机 CLI。库用 Argon2id 派生密钥去封装随机数据密钥，条目用 AES-256-GCM 加密，密文经 Google Drive 同步。`DefaultKDF()` 的参数是 64 MiB 内存、3 次迭代、2 线程（`tools/keyenv/internal/vault/crypto.go`）。setup 或 unlock 之后，未封装的数据密钥写在本机 `unlock-cache.json`；Google refresh token 用同一把数据密钥加密后留在本机。Drive 上没有主密码，也没有这把未封装的密钥（[设计第 6 节](../keyenv-design.md)、[CLI 说明](../../tools/keyenv/README.md)）。

这套模型依赖两件 cloud session 没有的东西：一台已经 unlock 的机器，以及一次人工浏览器授权。设计第 8 节已经写明，云端接入需要单独的授权和解锁材料，首版不向 Cloud Codex 发布凭据。同一节也写明，拥有本机执行权的 agent 可以读取子进程环境变量，工具做不到强隔离。skill 的约束是 agent 不把值读回聊天、参数、日志或 Git，秘密输入由用户在本机终端完成。

因此 cloud session 的问题不是“再装一个 keyenv”。新机器没有 unlock cache，也没有人做 Drive OAuth。就算把 cache 复制进去，那台临时机器和它上面的 agent 就等于拿到了解密整个库的材料。

## 2. 现有密钥管理方案

### Infisical

官方架构页写存储后端是 PostgreSQL，Redis 负责队列、定时任务和缓存（[architecture components](https://infisical.com/docs/internals/architecture/components)）。硬件要求页把 Redis 写成硬依赖：没有配置 Redis 连接就拒绝启动，Redis 不可达时实例降级；队列和协调状态在 Redis 里，要求 `noeviction` 并开启持久化（[self-hosting requirements](https://infisical.com/docs/self-hosting/configuration/requirements)）。

源码和这两页一致：

- `backend/src/db/knexfile.ts` 的 development 和 production 都是 `client: "postgres"`。
- `backend/src/lib/config/env.ts` 在解析环境时要求 `REDIS_URL`、`REDIS_SENTINEL_HOSTS`、`REDIS_CLUSTER_HOSTS` 三者至少有一个。
- `backend/src/main.ts` 启动时建立数据库连接、跑迁移、用 `queueServiceFactory` 建队列，再 `buildRedisFromConfig`。
- `backend/src/queue/queue-service.ts` 使用 BullMQ。
- `backend/package.json` 的 `engines.node` 是 `>=26.0.0`，依赖里有 `knex`、`pg`、`ioredis`、`bullmq`、`fastify`、`argon2`。
- `backend/src/db/migrations` 下匹配 `*.ts` / `*.mjs` 的迁移文件有 603 个。
- `backend/src/services` 有 131 个服务目录，`backend/src/ee/services` 有 79 个。

根目录 [LICENSE](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/LICENSE) 写明：任何 `ee/` 目录适用该目录里的 `ee/LICENSE`，其余是 MIT。`backend/src/ee/LICENSE.md` 允许在开发和测试时复制修改，生产使用要有有效的 Enterprise 订阅和对应席位；没有订阅时禁止复制、合并、发布、分发、再许可和销售。

Machine identity 在社区代码里。`backend/src/services/identity-ua/` 和 `backend/src/server/routes/v1/index.ts` 的 `registerIdentityUaRouter` 都不在 `ee/`。文档把 machine identity 定义成不需要用户账号的工作负载身份：用某种 auth method 换短期 access token，默认带 Universal Auth，客户端要用 client id 和 client secret 登录。吊销方式包括吊销 token、删除 identity、删除签发该 token 的 client secret。文档同时写明，吊销时如果 Redis 短暂不可用，被吊销的 token 最多还能用 12 分钟；要立刻失效就得删掉 client secret 或整个 identity（[Machine identities](https://infisical.com/docs/documentation/platform/identities/machine-identities)）。

未授权自托管的默认功能集在 `backend/src/ee/services/license/license-fns.ts` 的 `getDefaultOnPremFeatures()`：`identityLimit`、`memberLimit`、`environmentLimit` 都是 `null`，`dynamicSecret`、`secretRotation`、`auditLogs`、`rbac` 为 false，`secretsBrokering` 为 true，`agentVaultManagedStorage` 为 false。同文件注释写：Cloud 会在 `withCloudPlanDefaults` 里打开 managed storage，自托管要许可证才包含它。这次没有在 `agent-vault` 服务目录里搜到 `getPlan` 调用，所以不能说每一条 Agent Vault 路由都会在运行时检查许可证；生产使用的许可边界来自 `ee/LICENSE`，managed storage 的默认开关来自上述 feature set。

Cloud 免费档在文档导出的定价表里是：5 个 identity、无限项目、每个项目 3 个 environment、10 个 secret sync，价格为 0。identity 包含人和机器。自托管核心是 MIT，可自托管；SSO、LDAP、SCIM、KMIP、HSM、审计流出在付费档（[Infisical llms-full.txt](https://infisical.com/llms-full.txt)、[定价页](https://infisical.com/pricing)）。定价页的 FAQ 用了 “unlimited Free plan” 这种更宽的说法，具体上限以同一文档导出里的表为准。

`infisical run` 在 CLI 提交 `baf672b` 的 `packages/cmd/run.go` 里把拉到的秘密放进 `exec.Cmd` 的 `Env`，再启动子进程。`--command` 走 shell。同一文件有一条 debug 日志，把即将注入的环境变量切片传给 `log.Debug()`。这次没有追 zerolog 的默认级别，所以只记录源码里存在这条日志。Universal Auth 的 access token 和旧的 service token 都能走这条注入路径。

官方 MCP `@infisical/mcp`（提交 `d19e3ca`，`src/index.ts`）用 Universal Auth 的 client id/secret 或一个 access token 登录。`list-secrets` 和 `get-secret` 会把 `secretValue` 放进工具结果。`INFISICAL_MASK_SECRET_VALUES` 默认 false；设为 true 时，`secretValue` 被换成一个空格，名称和路径仍返回。仓库里还有一个未合并的 PR，提议 `list-secrets` 默认不带值（[PR #22](https://github.com/Infisical/infisical-mcp-server/pull/22)）。按已发布源码，遮罩是显式打开的，不是默认行为。

Agent Vault 是另一套产品，代码在 `backend/src/ee/services/agent-vault*` 和 `backend/src/ee/routes/v1/agent-vault-routers/`。文档描述的代理跑在 agent 外面：agent 访问真实的 MCP 或 HTTP 地址，代理从 Infisical 取出访问令牌并附上，agent 不持有该令牌；OAuth 刷新由 Infisical 做。文档建议代理和 agent 不要放在同一台机器，否则 agent 可能读到代理令牌和代理 CA 私钥（[How Agent Vault works](https://infisical.com/docs/documentation/platform/agent-vault/how-it-works)、[Proxies](https://infisical.com/docs/documentation/platform/agent-vault/proxies)、[MCP servers](https://infisical.com/docs/documentation/platform/agent-vault/mcp-servers)）。这和“本机 unlock cache”是同一类风险：代理材料如果出现在 agent 能读的文件系统里，隔离就没了。

### 1Password

Environments MCP 随桌面应用在本机以 stdio 运行。服务器只能看到 Environment 和变量名，不把秘密返回给客户端；连接和按 Environment 的操作会在桌面应用里要一次批准，批准持续到 1Password 锁定。文档列出的客户端是 Claude Code、Codex、Cursor、Kiro，并写明需要支持 stdio 的客户端，只支持远程传输的客户端目前不支持（[MCP server](https://www.1password.dev/environments/mcp-server)、[Cursor 插件](https://www.1password.dev/environments/cursor-plugin)、[Codex 插件仓库](https://github.com/1Password/1password-codex-plugin)）。

另一条产品是 1Password for Claude：用户在桌面应用里批准后，1Password 把凭据填进页面，Claude 只收到元数据和成败，看不到密码或一次性验证码。每次新会话都要重新批准，没有长期批准（[安全说明](https://support.1password.com/1password-claude-security/)）。这是有人在场的浏览器填充，不是无人值守的 cloud session。

`op run` 把 Environment 或 `op://` 引用注入子进程环境，进程结束后这些变量消失。文档明确说，同一用户下的其他进程仍可能读到该进程的环境（[Load secrets into the environment](https://www.1password.dev/cli/secrets-environment-variables)）。Service Account 用一个令牌（环境变量）做非交互登录，可以读被授权的 Environment，不能访问内置的 Personal、Private、Employee 和默认 Shared vault；Environments 只有读权限（[Service Accounts](https://www.1password.dev/service-accounts/get-started)）。

公开价目是 Teams Starter Pack 年付每月 24.95 美元起，Business 年付每用户每月 8.99 美元，Business 有 14 天试用（[Business pricing](https://1password.com/pricing/business)）。这次读到的 Service Account 文档和商务定价没有给出一个免费、可无人值守、且对远程虚拟机隐藏明文的档位。

### OpenBao 和 Vault

OpenBao 是 Vault 的分叉，许可证是 MPL-2.0（[仓库 LICENSE](https://github.com/openbao/openbao/blob/main/LICENSE)）。存储后端在加密屏障之外，数据先加密再写入。当前文档推荐在 Raft 集成存储和 PostgreSQL 之间选择，并写 PostgreSQL 对不熟悉 Raft 的人更好操作（[架构](https://openbao.org/docs/internals/architecture/)、[storage 说明](https://github.com/openbao/openbao/blob/main/website/content/docs/configuration/storage/index.mdx)）。file 和 inmem 不作为生产后端。它是一个要解封、要磁盘或外部数据库的服务，不是 Workers 脚本。

HashiCorp Vault 自 1.15.0 起使用 Business Source License，许可方是 IBM。附加使用授权允许生产使用，同时限制把软件以托管或嵌入方式提供给第三方、从而与 IBM 的付费版本竞争（[Vault LICENSE](https://github.com/hashicorp/vault/blob/main/LICENSE)）。个人自己跑一份 Vault 落在 BSL 的允许范围内，但许可证已经不是 OpenBao 那种 MPL。两者都要一台常驻机器。AppRole 或 token 仍然是 cloud session 上的引导秘密。

### Bitwarden Secrets Manager

免费组织包含核心功能：最多 2 个用户、3 个项目、3 个 machine account，秘密数量不限。Teams 每用户每月 6 美元，含 20 个 machine account；Enterprise 每用户每月 12 美元，含 50 个，自托管在 Enterprise（[方案说明](https://bitwarden.com/help/secrets-manager-plans/)、[产品页](https://bitwarden.com/products/secrets-manager/)）。Machine account 是发给程序的身份，用 access token 访问被分配项目里的秘密（[快速开始](https://bitwarden.com/help/secrets-manager-quick-start/)）。产品页描述端到端加密。token 仍然要放进 cloud session，进程拿到的是解密后的值。免费档的 3 个 machine account 够个人把 Cursor、Codex、Claude 分成三个可单独吊销的身份。

### Doppler

Developer 档写的是 3 个用户免费，超出后每用户每月 8 美元，包含 CLI、service token、3 天活动日志、5 个 config sync（[定价页](https://www.doppler.com/pricing)）。同一页的对比表在 Developer 列把 Access control 写成 25；[平台限制](https://docs.doppler.com/docs/platform-limits)也把 Developer 的 Users 写成 25、Projects 写成 10、Service Tokens 写成 50。这两处和“3 个免费用户”同时出现，页面没有解释免费席位和 25 的上限是不是同一件事，这里并列引用，不把它们合成一个数字。

Service token 默认可读一个 config，可加写权限和过期时间，吊销立即生效。`doppler run --` 把秘密注入随后那条命令的环境。文档要求不要把个人 CLI token 用在长期环境里（[Service Tokens](https://docs.doppler.com/docs/service-tokens)）。定价页列出 Official MCP Server。这次没有读该 MCP 的工具实现，所以不判断它是否默认把值返回给模型。

### SOPS 加 age

SOPS 是 MPL-2.0 的单二进制（[LICENSE](https://github.com/getsops/sops/blob/main/LICENSE)）。age 是 BSD 风格许可证（[age LICENSE](https://github.com/FiloSottile/age/blob/main/LICENSE)）。SOPS 用 age 收件人（公钥）加密数据密钥，解密时在本机找私钥：默认路径、`SOPS_AGE_KEY_FILE` 或 `SOPS_AGE_KEY`（[age 身份文档](https://github.com/getsops/docs/blob/main/content/en/docs/usage/identities/age/_index.md)）。没有服务器。公钥可以进 Git，私钥不行。换收件人用 `sops updatekeys`，旧私钥在重加密之前仍能解开旧文件。

对 cloud session，age 私钥就是引导秘密。放进平台的 Runtime Secret 之后，会话里的进程能解密仓库里的文件。没有中心吊销，只能轮换收件人并重加密。它适合“秘密已经在 Git 里、只差一把私钥”，不适合“会话里不出现解密材料”。

### dotenvx

仓库许可证是 BSD-3-Clause（[dotenvx](https://github.com/dotenvx/dotenvx/)）。`dotenvx encrypt` 把 `.env` 的值换成 `encrypted:` 密文，公钥留在文件里，私钥是 `DOTENV_PRIVATE_KEY`，本地默认写到不应提交的 `.env.keys`。`dotenvx run` 在启动进程前解密并注入环境变量。变量名保持明文（[加密后的 .env](https://dotenvx.com/docs/encrypted-env-file/)、[部署](https://dotenvx.com/docs/quickstart/deploy/)）。和 SOPS 一样，cloud session 必须先有私钥。GitHub Actions 文档的做法也是把私钥放进 Actions secret，再传给 `dotenvx run`（[GitHub Actions](https://dotenvx.com/docs/github-actions/)）。

### Phase

Phase Cloud 免费档：最多 5 个用户或 service account、3 个 app、dev/staging/prod 三个 environment，包含 CLI、SDK，以及定价表里的 “AI Agent Integration” 和 “Phase Agents Relay”。Relay 在功能表里的说明是 “AI egress credential proxy / connection broker”，免费档就有。自托管免费档把用户和 service account 写成不限，仍是 3 个 environment，并写没有出站用量遥测（[定价](https://phase.dev/pricing/)）。

自托管要求容器运行时，最低 2 核 CPU、2 GB 内存，后端和 worker 连接 PostgreSQL（[Self Hosting](https://docs.phase.dev/self-hosting)、[环境变量](https://docs.phase.dev/self-hosting/configuration/envars)）。`phasehq/console` 的 LICENSE 和 Infisical 同一结构：`ee/` 另有许可证，其余 MIT（[console LICENSE](https://github.com/phasehq/console/blob/main/LICENSE)）。这次没有审计 Relay 的源码，所以只采用定价页对它的那句功能描述，不判断 agent 是否完全看不到上游令牌。文档里的最低内存和 PostgreSQL 已经说明它不是免费 Workers 上的进程。

`phase run` 的文档用法是把列出的环境变量注入 `docker compose` 等进程，前提是会话里有 `PHASE_SERVICE_TOKEN`（[Docker Compose](https://docs.phase.dev/integrations/platforms/docker-compose)）。这仍然是一个引导令牌。

### Teller

Teller 是 Apache-2.0 的 CLI（[LICENSE](https://github.com/tellerops/teller/blob/master/LICENSE.txt)）。它不保存秘密，`.teller.yml` 指向 Vault、AWS Secrets Manager、Google Secret Manager、dotenv 等提供者。`teller run` 把取到的值注入子进程；`teller show` 每个值只显示前两个字符；`teller redact` 可以过滤输出（[README](https://github.com/tellerops/teller/blob/master/README.md)）。cloud session 仍然要有各个提供者的凭据。它适合已经有金库之后的注入和脱敏，不解决金库本身放在哪里。

## 3. 平台的插件、MCP 和登录态放在哪

问题是：能不能像本机 1Password 插件那样，由平台或服务端记住登录，使新的 cloud session 不必自带引导密钥。三条平台的答案不一样。

### Cursor

Cloud Agent 支持团队配置的 MCP，个人服务器从 `cursor.com/agents` 添加。HTTP 和 stdio 都可以，SSE 和 `mcp-remote` 不支持。OAuth 按用户，即使用的是团队共享服务器。

HTTP 服务器的配置不进入虚拟机。agent 拿不到刷新令牌、请求头和其他凭据，工具调用由后端代理。stdio 服务器跑在虚拟机里，agent 能读到它的配置和环境变量。敏感字段保存后不能再读回：stdio 的 `env`、HTTP 的 `headers`、OAuth 的 `CLIENT_SECRET`（[capabilities](https://cursor.com/docs/cloud-agent/capabilities)）。

本地和网页的 OAuth 回调是分开的。网页和 Cloud Agents 用 `https://www.cursor.com/agents/mcp/oauth/callback`，桌面用 `http://localhost:8787/callback`。Cloud Agents 使用自己的 OAuth 登录，不复用本机 Cursor 的登录（[MCP](https://cursor.com/docs/mcp)）。

这就是“平台保存登录态”的现成实现，范围是 HTTP MCP。它管不到必须在虚拟机里读取原始令牌的 CLI，例如把 `CLOUDFLARE_API_TOKEN` 放进进程环境的 wrangler。那种值要走下面第 4 节的 Runtime Secret，进程仍然能读到。

### Claude Code

本地远程 MCP 走 OAuth。云会话里的 connector 不在会话中打开登录流程，会话代理使用用户在 claude.ai 已经授予的授权；需要重新授权时去 claude.ai 的 connector 设置，而不是在会话里登录（[MCP](https://code.claude.com/docs/en/mcp)）。connector 流量经 Anthropic 的服务器，不必把对方主机加进会话的域名允许列表（[cloud environments](https://code.claude.com/docs/en/cloud-environments)）。

本机 Claude 登录凭据：macOS 优先钥匙串，钥匙串拒绝写入时落到 `~/.claude/.credentials.json`（模式 0600）；Linux 直接用这个文件；Windows 用用户目录下的同名文件（[Authentication](https://code.claude.com/docs/en/authentication)）。一篇对 Claude Code 2.1.257 的观察记录说，Linux 上的该文件里有 `mcpOAuth` 对象（[secretspec 的记录](https://secretspec.dev/blog/claude-code-stores-oauth-tokens-in-plaintext/)）。官方认证页描述的是登录凭据的存放位置，没有单独写 MCP 令牌的字段名，所以字段名只作为该版本的观察，不作为官方格式承诺。云会话不读本机这个文件，也不调用 `apiKeyHelper`。

因此 Claude 的云 connector 可以不在虚拟机里放引导密钥。自定义的、不是 claude.ai connector 的远程 MCP，仍要在交互会话里先完成 `/mcp` 登录；`claude -p` 和非交互运行不能自己弹出 OAuth。

### Codex 和 ChatGPT

Codex 客户端把 MCP OAuth 凭据放进操作系统钥匙串；钥匙串不可用时写到 `CODEX_HOME/.credentials.json`。`oauth.rs` 文件头写明这是为了和其他编码 CLI 一致（[oauth.rs](https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs)）。一台每次新建的 cloud 虚拟机没有这个钥匙串，也没有上一台机器的 `CODEX_HOME`。

ChatGPT 的自定义 MCP 在 OAuth 完成后，由 ChatGPT 把 access token 附到发往 MCP 服务器的请求上。验证签名、受众和 scope 是 MCP 服务器的责任（[Apps 认证](https://developers.openai.com/plugins/build/auth)）。添加方式支持 OAuth、无认证，以及在授权服务器公布能力时使用 CIMD 或 DCR（[Add custom MCP server](https://developers.openai.com/api/docs/guides/custom-mcp-server)）。

OpenAI 的应用安全白皮书写：插件本身不授予外部系统权限；需要认证时仍走底层 app、connector 或 MCP 的 OAuth，凭据应按平台或提供者的秘密处理，不要写进提示词（[App Security Whitepaper](https://cdn.openai.com/business-guides-and-resources/app-security-whitepaper.pdf)）。

有一份对公开客户端代码的问题描述说，从 Codex App 安装带远程 MCP 的官方插件会在 ChatGPT 账户上建立 connector 授权，工具经 `codex_apps` 远程服务器提供，本机不落每个 connector 的令牌；本地关掉插件不等于吊销账户授权（[openai/codex#30023](https://github.com/openai/codex/issues/30023)）。这是问题报告里对源码的阅读，不是 OpenAI 的产品文档。另一份仍开放的需求要求 Codex Cloud 提供插件和 connector 的选择与 `@` 提及，并说明云任务里到底能调用什么（[openai/codex#54258](https://github.com/openai/codex/issues/54258)）。当前云环境文档写了环境变量和 HTTP secret，没有写 cloud 任务会复用 ChatGPT 里已经授权的 Apps。所以：账户级 connector 的登录态在服务端这一点有客户端代码和白皮书支持；Codex Cloud 任务能否直接用这些 connector，这次没有找到官方说明。

1Password 那种“本机弹批准、值不进模型”的插件，在 Codex 上也是本机 stdio，依赖桌面应用。它不随 cloud 虚拟机出现。

## 4. 各 cloud session 怎么提供 secret，网络怎么限制

### Cursor Cloud Agents

秘密在 Cloud Agents 的 Secrets 里，启动时注入。已经在跑的 agent 不会看到后加的秘密。三种类型（[Secrets & Network](https://cursor.com/docs/cloud-agent/security-network)）：

| 类型 | 会话里的形态 | 模型看不看得到 |
| --- | --- | --- |
| Environment Variable | 环境变量 | 看得到，适合非秘密配置 |
| Runtime Secret | 仍然是环境变量 | 工具结果、聊天记录、提交和提交说明里的内容会被换成 `[REDACTED]`。终端里的用户仍能看到 |
| Build Secret | 只在 Docker 构建的 secret mount 里 | 不进入运行中的 agent 环境 |

Runtime Secret 是输出脱敏，不是进程隔离。agent 的进程环境里有这串字符。文档还建议能用 OIDC 的云角色不要放长期密钥：agent 可以从本机套接字拿 Cursor 签名的 JWT，交给 AWS、GCP、Azure 或 Vault。

网络默认可达外网。可以改成“默认域名加允许列表”或“只允许列表”。只允许列表时，Cursor 自己的服务和源代码托管仍然可达。Self-Hosted Machine 用那台机器自己的网络。私网可以用 Tailscale 或 Cloudflare Tunnel，并用 Access service token 保护主机名，token 本身放在 Cursor Secrets 里。

`.cursor/environment.json` 没有秘密字段。快照会保存构建时放进磁盘的文件；文档建议秘密走 Secrets，而不是写进会被快照的 `.env.local`（[环境设置](https://cursor.com/docs/cloud-agent/setup)）。

### OpenAI Codex cloud

要分开两套文档。

现行云环境：环境变量直接传给环境里的程序。Network secret（页面上也叫 HTTP secret）让程序看到占位符，代理只在发往允许域名的 HTTPS 443 请求里替换真实值，setup 和任务阶段都做替换，原始值不进入本机进程或文件。如果程序必须读到原始值，就要改配成环境变量。保存属于环境的 network secret 会把目的地加进受限网络；直接的环境变量不会。个人保险库可以按环境提供值，只有被请求的值会进入任务（[Cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environments)）。

旧版 Codex Cloud（页面标题带 Legacy）：环境变量覆盖 setup 和 agent 全程；名为 secrets 的那一类只给 setup 脚本，agent 阶段开始前移除。setup 脚本和 agent 不是同一个 shell，`export` 不会保留到 agent 阶段（[Codex Cloud (Legacy)](https://learn.chatgpt.com/docs/environments/cloud-environment)）。OpenAI 的安全说明把这两套并列写了出来，并指出现行行为以 cloud environments 页为准（[llms-full.txt 中的安全段](https://learn.chatgpt.com/docs/llms-full.txt)）。

所以“秘密只在 setup 可用”描述的是旧版 secrets。现行产品里，network secret 在任务阶段也可用，而且是这三家里最接近“进程拿不到原始值”的内置机制，限制是只替换 HTTPS 443 上允许域名的请求。wrangler、`gh` 这类要自己读令牌的程序用不上这条路径。

### Claude Code on the web

每个会话是隔离虚拟机。Anthropic 托管时，git 凭据留在沙箱外，由代理用范围受限的凭据代认证。加到云环境里的 network secret 同样留在沙箱外，请求离开会话后再附上。自托管环境没有 network secret（[Claude Code on the web](https://code.claude.com/docs/en/claude-code-on-the-web)）。

网络四级：None、Trusted（包仓库、GitHub、云 SDK 等允许列表）、Full、Custom。即使选 None，会话仍能到达 GitHub 代理、已启用的 MCP connector、network secret 的主机，以及 Anthropic API（[cloud environments](https://code.claude.com/docs/en/cloud-environments)）。改允许列表后，已有会话大约一分钟内按新设置走那些经过允许列表的请求。

### GitHub Actions 和 Claude Code action

仓库、环境和组织 secret 在作业里通过 `secrets` 上下文变成动作输入或环境变量，覆盖整个作业，不是只在安装阶段。fork 触发的工作流拿不到这些 secret（`GITHUB_TOKEN` 除外）。文档警告：不是 GitHub secret 的敏感值要用 `::add-mask::` 才会按秘密打码；同时要求不要在命令行参数里传递秘密（[Using secrets in GitHub Actions](https://docs.github.com/en/actions/security-guides/using-secrets-in-github-actions)）。支持 OIDC 的云厂商可以不存放长期密钥。

Claude Code GitHub Action 使用仓库里的 Anthropic 凭据或 `claude setup-token` 产生的长期 OAuth 令牌。这个令牌是给 CI 的引导秘密，交互式云会话的 connector 登录不能代替它（[GitHub Actions](https://code.claude.com/docs/en/github-actions)、[Authentication](https://code.claude.com/docs/en/authentication)）。

### Devin

组织级、仓库级、企业级秘密加密存储，会话级秘密只在当前会话。秘密不会导出到每一个 shell；浏览器里由 Devin 直接输入，shell 命令在需要时绑定环境变量。企业级可以标记 build-only，快照完成后、克隆仓库前移除，会话里不可用（[Secrets](https://docs.devin.ai/product-guides/secrets)、[企业环境](https://docs.devin.ai/enterprise/environment-management/overview)）。网络由安全配置的允许列表控制，作用于 shell、浏览和安装；MCP 是否可用由另一张允许列表决定，不必把 MCP 主机写进网络允许列表（[Security Profiles](https://docs.devin.ai/product-guides/security-profiles)）。Devin 是付费产品，不在“只用免费额度”的候选部署里，这里只作为平台行为对照。

## 5. Cloudflare 免费额度能支撑什么

账户是 Workers Free。付费计划的最低月费是 5 美元，下面凡是写“付费”的都不在当前约束里（[Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/)）。

| 产品 | 免费计划里实际可用的部分 | 和这个需求的关系 |
| --- | --- | --- |
| Workers | 每天 10 万次请求，单次 HTTP 10 ms CPU，128 MB 内存，每次请求 50 次子请求、最多 6 个同时出站连接。环境变量每个 Worker 64 个、每个 5 KB。超过免费上限后该类操作失败，不是自动扣费（[limits](https://developers.cloudflare.com/workers/platform/limits/)） | 给一次请求加上认证头、做一次 AES-GCM，CPU 预算够。Argon2id 按 keyenv 的 64 MiB、3 次迭代跑，不适合放在这 10 ms 里。WebCrypto 也没有 Argon2 |
| D1 | 每天 500 万行读、10 万行写，账户存储合计 5 GB，最多 10 个库、每库 500 MB，Time Travel 7 天。自 2026-09-01 起超限查询直接失败，数据还在（[D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/)、[limits](https://developers.cloudflare.com/d1/platform/limits/)、[执行变更](https://developers.cloudflare.com/changelog/post/2026-09-01-d1-free-tier-limit-enforcement/)） | 个人规模的密文和审计足够。它是 SQLite，不是 PostgreSQL，没有 Infisical 用的 advisory lock 和那 603 个 PG 迁移 |
| Durable Objects | 免费计划只能建 SQLite 后端。请求每天 10 万，时长每天 1.3 万 GB-s，行读写和 D1 同一量级，SQL 数据合计 5 GB（[DO pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/)） | 适合一把锁、一次令牌刷新这种单线程协调。不适合拿来跑 Infisical |
| KV | 每天 10 万次读、1 千次写、1 千次删除、1 千次 list，存储 1 GB（[Workers pricing 的 KV 表](https://developers.cloudflare.com/workers/platform/pricing/)） | 写配额小，不适合当会频繁轮换的库。最终一致也不适合当令牌吊销的唯一记录 |
| Secrets Store | 公开测试。每账户一个 store、最多 100 条生产秘密，单条最长 65536 字节。写入后不能经 API 或控制台再读出，由绑定的 Worker 或 AI Gateway 使用（[概述](https://developers.cloudflare.com/secrets-store/)、[管理](https://developers.cloudflare.com/secrets-store/manage-secrets/)） | 比 Worker 环境变量的 5 KB 宽。文档没有另列美元价格。本地 `wrangler` 不带 `--remote` 的秘密以明文留在 `.wrangler/state` |
| Access service token | 账户限制 50 个 service token，不按套餐拆开写（[account limits](https://developers.cloudflare.com/cloudflare-one/account-limits/)）。当前套餐页的 Zero Trust Free 是最多 50 个用户（[Cloudflare plans](https://www.cloudflare.com/plans/)）。2022 年的套餐表把 service token 认证标在 Free 列（[2022 PDF](https://www.cloudflare.com/static/3fb3993535599c90e3fb6b64f2c11d67/Cloudflare_Zero_Trust_Pricing___Plans__Q4_2022__.pdf)）。这次没有找到把 service token 改成付费专属的更新文档 | token 是一对 Client ID 和 Client Secret，只在创建时显示一次，可以按条吊销（[Service Tokens](https://developers.cloudflare.com/cloudflare-one/access-controls/service-credentials/service-tokens/)）。它本身就是引导秘密 |
| Tunnel | 每账户 1000 条隧道不另收费，超出后的 Tunnels for Platforms 是每条每月 4.50 美元（[account limits](https://developers.cloudflare.com/cloudflare-one/account-limits/)、[Tunnels for Platforms](https://developers.cloudflare.com/tunnel/features/tunnels-for-platforms/)） | 可以把家里的机器暴露成 Access 后面的主机名。电脑关机时，cloud session 就到不了那台机器。这解决不了“人没在、机器也没开” |
| Containers | 定价表的 Free 列是 N/A。计费挂在每月至少 5 美元的 Workers Paid 上，按运行中的内存、vCPU 和磁盘计（[Containers pricing](https://developers.cloudflare.com/containers/platform/pricing/)） | 不在免费额度里 |
| 容器磁盘 | 磁盘默认是临时的。实例睡眠后再次启动，用的是镜像里的新磁盘。要保留文件系统必须在 `durable_object` 调度策略下自己做快照并在下次 `start()` 时恢复。快照不可变，不包含内存和进程，保留 30 天，恢复一次刷新这 30 天（[FAQ](https://developers.cloudflare.com/containers/faq/)、[快照](https://developers.cloudflare.com/containers/guides/snapshots/)） | 即使以后付费，也不能把 OpenBao 的 Raft 或 Infisical 的 Postgres 数据目录放在这块默认磁盘上 |

`miles1150.top` 可以做 Worker 的自定义域名。免费计划没有提供 PostgreSQL 或 Redis 服务。Hyperdrive 出现在免费计划的包含项里，但它连接的是外部数据库；没有 VPS 或其他托管 Postgres 时，它不能把 Infisical 带起来。

## 6. 对比

“agent 能否看到明文”按最贴近目标的用法填写。进程环境里有值，就写成“进程能读到”；只有输出被打码，会单独注明。

| 方案 | 部署 | 免费程度 | 无人值守的 cloud session | agent 与明文 | 吊销 | Cloudflare 免费计划 |
| --- | --- | --- | --- | --- | --- | --- |
| keyenv 现状 | 本机 CLI，Drive 存密文 | 自己的 Google 账号 | 新机器没有 unlock cache，也没有人做 OAuth | 子进程环境有值；工具输出默认没有 | 停用条目，不吊销厂商令牌；`lock` 只删本机 cache | 不能当服务跑 |
| Infisical Cloud Free | Infisical 托管 | 5 个 identity、3 个 environment | 可以，虚拟机里要有 Universal Auth 的 client secret | `infisical run` 注入子进程。MCP 默认返回 `secretValue`，遮罩要显式打开 | 删 client secret 或 identity。只吊销 token 时，Redis 抖动下最多还能用 12 分钟 | 不需要自建 |
| Infisical 自托管 CE | Postgres + Redis + Node 26 | 软件 MIT，机器要自己有 | 同样要 client secret | 同上 | 同上，外加自己管 Redis | 跑不起来 |
| Infisical Agent Vault | `ee/` 里的服务加一台代理 | 生产使用要 Enterprise 订阅 | 代理若在另一台机器，agent 可以不持有上游令牌；代理令牌仍是引导秘密 | 文档设计是 agent 不收到上游令牌 | 停会话、撤 OAuth、删代理令牌 | 代理是常驻进程，不是 Worker |
| 1Password Environments MCP | 桌面应用，stdio | 跟随 1Password 订阅；读到的商务档是付费加试用 | 远程客户端不被支持，cloud 虚拟机里没有桌面应用 | 工具不返回值；批准在桌面应用 | 锁上 1Password 即结束已批准的使用 | 不能 |
| 1Password service account + `op run` | SaaS，CLI 注入 | 读到的是 Teams / Business 价格 | 可以，虚拟机里要有 service account token | 子进程环境有值，同用户的其他进程可能读到 | 撤掉 service account 或缩小 vault 权限 | 不需要自建 |
| OpenBao | 自备机器，Raft 或 Postgres | 软件 MPL-2.0，机器自备 | 要 token 或 AppRole secret id | 拿到响应的进程能看到值 | 吊销 token、改 policy | 不能 |
| Vault 1.15+ | 同上 | BSL，个人自用在附加授权里，托管给别人有竞争限制 | 同 OpenBao | 同 OpenBao | 同 OpenBao | 不能 |
| Bitwarden Secrets Manager Free | Bitwarden 托管；自托管在 Enterprise | 2 用户、3 项目、3 个 machine account | 可以，要有 machine account 的 access token | 进程拿到解密后的值 | 删 token 或 machine account | 不需要自建 |
| Doppler Developer | Doppler 托管 | 定价页写 3 个用户免费；平台限制页写 25 用户、50 个 service token | 可以，要有 service token | `doppler run` 注入子进程。官方 MCP 的返回值这次没读源码 | 吊销 token 立即生效 | 不需要自建 |
| SOPS + age | Git 里的密文，本机私钥 | 软件免费 | 虚拟机里要有 age 私钥 | 解密后的值在进程里 | 移除收件人并 `updatekeys`；旧密文在重加密前仍能被旧私钥解开 | 不能当服务 |
| dotenvx | Git 里的加密 `.env` | BSD-3 | 虚拟机里要有 `DOTENV_PRIVATE_KEY` | `dotenvx run` 注入环境；变量名是明文 | 轮换密钥并重新加密 | 不能当服务 |
| Phase Free | Cloud，或自托管 Postgres | Cloud：5 个用户或 service account、3 个 app。自托管免费档用户不限，仍要机器 | service token 是引导秘密。Relay 被标成出站凭据代理，源码未读 | `phase run` 注入列出的变量 | 撤 service account。Relay 的吊销语义未读源码 | 自托管不是 Worker |
| Teller | 本机 CLI，背后是别的金库 | Apache-2.0 | 要有背后金库的凭据 | `teller run` 注入；`show` 只显示前两个字符 | 吊销背后的凭据 | 不能当服务 |
| Cursor HTTP MCP | Cursor 后端代理 | 随 Cursor 账号 | 用户在控制台登录一次，之后的 cloud 会话不把刷新令牌放进虚拟机 | 工具结果仍可能含上游返回的数据；MCP 凭据本身不在虚拟机 | 在 Cursor 或上游撤掉 OAuth | 若 MCP 服务器自己用 Worker，可以 |
| Cursor Runtime Secret | Cursor 托管 | 随 Cursor 账号 | 可以，值在进程环境里 | 模型侧输出被换成 `[REDACTED]`；终端用户看得到；进程读得到 | 从 Secrets 删除，新开会话生效 | 不需要自建 |
| Codex network secret | OpenAI 代理替换 HTTPS | 随 ChatGPT / Codex | 任务阶段可用，程序只看到占位符 | 原始值不进入进程或文件；只覆盖允许域名的 443 | 从环境配置删除 | 不需要自建 |
| Codex 环境变量 | 直接进入环境 | 同上 | 任务全程都在 | 程序和能打印环境的 agent 能读到 | 从配置删除 | 不需要自建 |
| Claude connector | Anthropic 代持授权 | 随 Claude 账号 | 云会话不重新登录 | 工具结果由 connector 决定；授权材料不在沙箱里做 OAuth | 在 claude.ai 断开 connector | 不需要自建 |
| Claude network secret | 请求离开沙箱后再附加 | 随 Claude 账号 | 可以，秘密不在沙箱内 | 匹配请求在沙箱外被附上秘密 | 从云环境删除 | 不需要自建 |
| 自建 Worker 代理 | `miles1150.top` 上的 Worker，密文在 D1 | Workers / D1 免费额度内 | Cursor / Claude 用平台 OAuth 时，虚拟机可以不放长期密钥；Codex 需配合成 network secret 或环境变量 | 由工具设计决定：元数据工具不返回值；代发 HTTP 时上游令牌留在 Worker | 删授权行、轮换 Worker Secret、撤 Access token | 这是它的目标运行位置 |

## 7. 把 Infisical 改到 Cloudflare 上：不可行

判断依据是上面第 2 节的源码，不是“Node 服务一般不好搬”。

要保留的行为包括：用户和 machine identity 登录、秘密的读写、`infisical run` 能对着的 API、吊销立刻或在文档所述的 12 分钟窗口内生效。这些行为今天依赖：

- Knex 的 PostgreSQL 驱动和 603 个迁移。D1 和 Durable Object 的存储都是 SQLite。把迁移改写成 SQLite，等于重写数据层，还要换掉 PostgreSQL advisory lock。`keystore.ts` 的 `PgSqlLock` 用 `pgAdvisoryLockHashText` 生成锁 id，`org-service.ts` 里能看到 `SELECT pg_try_advisory_xact_lock(?)`。
- 启动期强制的 Redis，以及 BullMQ 的常驻 worker。Workers 没有可以连上去的 Redis，免费计划也不提供替代的持久队列进程。KV 的最终一致和每天 1000 次写不能替换 Infisical 用来做吊销缓存、锁和队列的 Redis。
- Fastify 进程、Node.js 26、原生 `argon2` 模块。免费 Worker 每次请求 10 ms CPU、128 MB 内存。
- Agent Vault 整套在 `ee/`。按 `ee/LICENSE`，没有 Enterprise 订阅就不能把它用于生产。自建时若删掉 `ee/`，就失去文档里那套“代理代持令牌”的能力，剩下的是社区版的秘密 API。社区版 API 仍然是上面这个 Node 服务。

工作量因此是：新的认证和数据模型、新的加密运行时、新的队列和吊销，再加上一套没有上游可合并的补丁。个人免费额度下，这个结果和“写一个只做元数据与代发的小 Worker”是同一量级，却多背 600 多个迁移和一份专有许可。Infisical Cloud 的免费档已经提供 5 个 identity；要的是现成产品时，用那个档，而不是在 Workers 里再实现一遍。

Phase 的自托管同样要 PostgreSQL 和至少 2 GB 内存，不构成另一条“搬上 Workers”的近路。OpenBao 要磁盘上的 Raft 或一台 Postgres。Containers 不在免费计划里，而且默认磁盘在睡眠后丢弃。

## 8. 三个可用架构

三个都承认同一件事：没有人在场时，主密码不能成为每次请求的解锁因素。keyenv 现在的做法是把解开的数据密钥缓存在已经解锁的 Mac 上。cloud session 若要在 Mac 关机时工作，这把可用的密钥必须放在平台或一台总是开着的服务里。放进去之后，那个平台就能解密。这和 Drive 上只有密文、本机才有数据密钥，是两种信任模型。

### 架构 A：各平台自己的 secret 和 connector

Mac 继续用 keyenv。cloud 真正要用的少数令牌，按调用方式拆开存放。

- 只通过 HTTPS 调用固定主机的令牌：Codex 用 network secret，写上允许的域名；Claude 用云环境的 network secret。程序拿不到原始值。
- Cursor 上能做成 HTTP MCP 的系统：在 Cloud Agents 的 MCP 里做一次 OAuth。刷新令牌留在 Cursor 后端，虚拟机里没有。
- Claude 上已有 claude.ai connector 的系统：在 claude.ai 授权一次，云会话由代理使用该授权。
- 必须出现在进程环境里的 CLI 令牌：Cursor 用 Runtime Secret；Codex 和 Claude 用它们的环境变量。范围收窄到这一条命令需要的权限，并接受进程能读到值、Cursor 只保证输出里的字面量会被换成 `[REDACTED]`。

能复用的 keyenv 部分：条目和字段名的元数据习惯、skill 里“不把值读回聊天、参数、日志和 Git”的操作约束、`run` 这种只向指定子进程注入的用法。Mac 上的 Argon2id、unlock cache、Drive OAuth 和加密备份继续服务本机，不复制到 cloud 虚拟机。

优点：没有新服务，不碰 5 美元的付费线，Cursor 和 Claude 的 HTTP/connector 路径可以不在虚拟机里放长期密钥。缺点：三份控制台，吊销要到每个平台和每个令牌的发行方各做一次。Codex Cloud 能否复用 ChatGPT connector，这次没有官方文档。必须进进程的 CLI 仍然看得到值。

### 架构 B：自己在 Worker 上做一个很小的凭据代理

在 `miles1150.top` 上放一个 Worker。密文和条目元数据放在 D1。用来解开这些密文的数据密钥放在 Worker Secret 或 Secrets Store，不在请求里跑 Argon2id。Mac 上如果仍要主密码，就在本机用现有 keyenv 解密，再把已经用这把数据密钥封装好的条目上传；云端请求只做 AES-GCM。

对 Cursor 和 Claude，把这个 Worker 做成远程 MCP，用平台 OAuth。登录态在平台，cloud 虚拟机不预置 client secret。工具只返回条目名、字段名和用途。需要调用上游 HTTP API 时，由 Worker 附上授权头再转发，行为和 Codex network secret、Infisical Agent Vault 的代理相同，上游令牌不进虚拟机。

对必须在虚拟机里读令牌的 CLI，Worker 做不到“注入别人的进程环境”。这一类仍要一条短命、可吊销的引导凭据：Cursor 放 Runtime Secret，Codex 优先放 network secret 让代理替换发往 `miles1150.top` 的 HTTPS。虚拟机里的小 CLI 取回值后执行子进程，skill 禁止打印。这是 keyenv `run` 的云端对应物，隔离强度停在 keyenv 设计第 8 节已经承认的那一层。

能复用的部分：`crypto.go` 的 AES-GCM 封装、随机 id、条目与字段分离、列表和查看只出元数据。版本对象和父版本冲突可以留到有两台写入者之后再搬。不复用 Drive 同步、OAuth 缓存和请求路径上的 Argon2id。

优点：一个吊销点，域名和免费额度都够个人用量，Cursor/Claude 可以不把长期上游令牌放进虚拟机。缺点：Cloudflare 能使用 Worker Secret，零知识模型在云端这一段不再成立。Codex Cloud 的自定义 MCP 没有官方文档。代理覆盖不了所有 CLI。认证、审计和备份要自己做，D1 免费库只有 7 天 Time Travel。

### 架构 C：用一家免费托管金库，不自建也不移植

选 Infisical Cloud Free、Doppler Developer 或 Bitwarden Secrets Manager Free 里的一个。只建一个机器身份，client secret 或 access token 放进架构 A 的 Runtime Secret 或环境变量。日常用对应的 `run` 注入子进程。

若选 Infisical，MCP 必须设置 `INFISICAL_MASK_SECRET_VALUES=true`，并尽量用 `INFISICAL_ENABLED_TOOLS` 收成只读。默认配置会把 `secretValue` 放进模型上下文。Agent Vault 不在这个架构里，因为它的生产许可和常驻代理都不在免费约束内。

能复用的 keyenv 部分几乎只有操作约束和“按名称选条目、向子进程注入”的流程。加密格式和 Drive 不同步，两套库会分叉，除非以后写一次从 Mac 导出再导入的迁移。

优点：吊销、界面和 CLI 是别人维护的，免费档够一个人加一两个机器身份。缺点：引导令牌仍在 cloud 环境里；服务商能解密；Infisical 的 5 个 identity、Bitwarden 的 3 个 machine account 是硬上限。和 keyenv 的“Drive 上看不到明文”不是同一个模型。

### 怎么选

先做架构 A，把现在 cloud 会话实际要调用的令牌分成“HTTPS 代发”和“进程必须读取”两类。只有当令牌开始散落在三个控制台、吊销容易漏的时候，再做架构 B。架构 C 适合想要现成审计界面、并且接受服务商能解密的时候。三条都不包括移植 Infisical。

## 9. 功能清单

### 现在就能做的一档

不写新服务。

- 列出 cloud 会话真正要调用的秘密，标成“只走 HTTPS”或“CLI 必须读环境变量”。
- HTTPS 那一类放进 Codex network secret 和 Claude network secret，允许域名写到具体主机。
- Cursor 上能提供 HTTP MCP 的，走控制台 OAuth，不把刷新令牌写进 Secrets。
- 必须进进程的，用发行方能单独吊销的令牌，放进 Cursor Runtime Secret 或对应平台的环境变量。每个平台一把，不复用 Mac 上的主密码和 unlock cache。
- 继续用现有 skill：不把值读回聊天、不写进命令参数、不提交。
- 写一页吊销说明：每个平台的秘密在哪里删，上游令牌在哪里轮换。删 Cursor Secret 之后要新开 agent。

### 下一档：若做架构 B

- Worker 自定义域名、D1 里的密文条目、一把数据密钥放在 Worker Secret 或 Secrets Store。
- 远程 MCP：列出和查看只返回名称、字段名、用途、状态。
- 一个代发动作，只允许预先登记的 HTTPS 主机，由 Worker 附加授权头。
- Cursor 和 Claude 用平台 OAuth 连接这个 MCP。
- 吊销是删除授权或轮换数据密钥。轮换时 Mac 上重新封装条目。
- 请求路径不做 Argon2id，不做 Drive 同步。

### 再往后

- 从已解锁的 keyenv 库做一次导入，导入在 Mac 上解密，上传的是新封装。
- D1 里记谁在何时取用了哪一条，不记值。
- 给必须进进程的 CLI 发短时凭证，过期后 Worker 拒绝。
- 多台写入者再引入 keyenv 已有的父版本和冲突，不在代理里自动选一个秘密版本。
- 上游 OAuth 的 refresh token 只放在 Worker，会话代理拿 access token。这是 Agent Vault 文档里的模型，用自己的少量代码实现，不搬 `ee/`。
- Codex Cloud 若以后有官方的自定义 MCP 或 connector 说明，再把架构 B 的登录态交给 Codex，而不是往虚拟机里放 client secret。

## 10. 没有核对到的边界

- Doppler 定价页列出了 Official MCP Server。这次没有读它的工具实现，不知道默认是否返回秘密值。
- Phase Agents Relay 只采用了定价页的功能描述，没有读代理源码。
- Codex Cloud 任务能否调用已经在 ChatGPT 授权的 Apps，云环境文档没有写。`#54258` 仍是开放的功能需求。`#30023` 是用户对客户端源码的描述。
- Infisical 默认 feature set 里的 `rateLimits` 数字是 read 60、write 200、secrets 40。这次没有顺着调用点确认时间窗口。
- 1Password 的个人或家庭订阅是否包含可无人值守的 Service Account，这次读到的上手文档和商务定价没有给出免费档。
- `@infisical/mcp` 的遮罩默认值以提交 `d19e3ca` 为准。未合并的 PR #22 如果以后合并，默认行为会变。

## 11. 引用

keyenv：

- [设计记录](../keyenv-design.md)
- [CLI 说明](../../tools/keyenv/README.md)
- [skill](../../plugins/keyenv/skills/keyenv/SKILL.md)
- `tools/keyenv/internal/vault/crypto.go` 的 `DefaultKDF`

Infisical：

- [架构组件](https://infisical.com/docs/internals/architecture/components)
- [自托管硬件要求](https://infisical.com/docs/self-hosting/configuration/requirements)
- [后端目录说明](https://infisical.com/docs/contributing/platform/backend/folder-structure)
- [根 LICENSE，提交 f943528](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/LICENSE)
- [ee/LICENSE.md](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/backend/src/ee/LICENSE.md)
- [knexfile.ts](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/backend/src/db/knexfile.ts)
- [env.ts 中 Redis 必填](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/backend/src/lib/config/env.ts)
- [main.ts](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/backend/src/main.ts)
- [license-fns.ts 的默认功能集](https://github.com/Infisical/infisical/blob/f943528f8f51d469fa6ef4ccb62d84d14b291cb8/backend/src/ee/services/license/license-fns.ts)
- [Machine identities](https://infisical.com/docs/documentation/platform/identities/machine-identities)
- [CLI run.go，提交 baf672b](https://github.com/Infisical/cli/blob/baf672b6b4296492d842601de7b79066e3484d72/packages/cmd/run.go)
- [MCP index.ts，提交 d19e3ca](https://github.com/Infisical/infisical-mcp-server/blob/d19e3ca244d0c1b2506d39803a73e6e2c3730efa/src/index.ts)
- [MCP 默认遮罩提案，未合并](https://github.com/Infisical/infisical-mcp-server/pull/22)
- [Agent Vault 工作方式](https://infisical.com/docs/documentation/platform/agent-vault/how-it-works)
- [Agent Vault 代理](https://infisical.com/docs/documentation/platform/agent-vault/proxies)
- [Agent Vault 的 MCP 服务](https://infisical.com/docs/documentation/platform/agent-vault/mcp-servers)
- [文档导出的定价表](https://infisical.com/llms-full.txt)
- [定价页](https://infisical.com/pricing)

1Password：

- [Environments MCP](https://www.1password.dev/environments/mcp-server)
- [Cursor 插件](https://www.1password.dev/environments/cursor-plugin)
- [Codex 插件](https://github.com/1Password/1password-codex-plugin)
- [1Password for Claude 的安全说明](https://support.1password.com/1password-claude-security/)
- [op run](https://www.1password.dev/cli/secrets-environment-variables)
- [Service Accounts](https://www.1password.dev/service-accounts/get-started)
- [商务定价](https://1password.com/pricing/business)

OpenBao、Vault、其他 CLI：

- [OpenBao LICENSE](https://github.com/openbao/openbao/blob/main/LICENSE)
- [OpenBao 架构](https://openbao.org/docs/internals/architecture/)
- [OpenBao storage](https://github.com/openbao/openbao/blob/main/website/content/docs/configuration/storage/index.mdx)
- [Vault LICENSE](https://github.com/hashicorp/vault/blob/main/LICENSE)
- [Bitwarden Secrets Manager 方案](https://bitwarden.com/help/secrets-manager-plans/)
- [Bitwarden machine account 快速开始](https://bitwarden.com/help/secrets-manager-quick-start/)
- [Doppler 定价](https://www.doppler.com/pricing)
- [Doppler 平台限制](https://docs.doppler.com/docs/platform-limits)
- [Doppler service tokens](https://docs.doppler.com/docs/service-tokens)
- [SOPS 的 age 身份](https://github.com/getsops/docs/blob/main/content/en/docs/usage/identities/age/_index.md)
- [SOPS LICENSE](https://github.com/getsops/sops/blob/main/LICENSE)
- [age LICENSE](https://github.com/FiloSottile/age/blob/main/LICENSE)
- [dotenvx](https://github.com/dotenvx/dotenvx/)
- [dotenvx 加密文件](https://dotenvx.com/docs/encrypted-env-file/)
- [dotenvx 部署](https://dotenvx.com/docs/quickstart/deploy/)
- [Phase 定价](https://phase.dev/pricing/)
- [Phase 自托管](https://docs.phase.dev/self-hosting)
- [Phase 数据库环境变量](https://docs.phase.dev/self-hosting/configuration/envars)
- [Phase console LICENSE](https://github.com/phasehq/console/blob/main/LICENSE)
- [Phase 的 docker compose 注入](https://docs.phase.dev/integrations/platforms/docker-compose)
- [Teller README](https://github.com/tellerops/teller/blob/master/README.md)
- [Teller LICENSE](https://github.com/tellerops/teller/blob/master/LICENSE.txt)

平台：

- [Cursor Cloud Agent capabilities](https://cursor.com/docs/cloud-agent/capabilities)
- [Cursor MCP](https://cursor.com/docs/mcp)
- [Cursor Secrets 与网络](https://cursor.com/docs/cloud-agent/security-network)
- [Cursor 环境设置](https://cursor.com/docs/cloud-agent/setup)
- [Claude Code MCP](https://code.claude.com/docs/en/mcp)
- [Claude Code 认证](https://code.claude.com/docs/en/authentication)
- [Claude Code on the web](https://code.claude.com/docs/en/claude-code-on-the-web)
- [Claude 云环境](https://code.claude.com/docs/en/cloud-environments)
- [Claude Code GitHub Actions](https://code.claude.com/docs/en/github-actions)
- [Codex MCP OAuth 源码](https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs)
- [ChatGPT 自定义 MCP](https://developers.openai.com/api/docs/guides/custom-mcp-server)
- [ChatGPT Apps 认证](https://developers.openai.com/plugins/build/auth)
- [OpenAI 应用安全白皮书](https://cdn.openai.com/business-guides-and-resources/app-security-whitepaper.pdf)
- [Codex 账户级 connector 的问题描述](https://github.com/openai/codex/issues/30023)
- [Codex Cloud 插件选择的功能需求](https://github.com/openai/codex/issues/54258)
- [现行 Codex 云环境](https://learn.chatgpt.com/docs/environments/cloud-environments)
- [旧版 Codex Cloud](https://learn.chatgpt.com/docs/environments/cloud-environment)
- [GitHub Actions secrets](https://docs.github.com/en/actions/security-guides/using-secrets-in-github-actions)
- [Devin secrets](https://docs.devin.ai/product-guides/secrets)
- [Devin 安全配置](https://docs.devin.ai/product-guides/security-profiles)

Cloudflare：

- [Workers 定价](https://developers.cloudflare.com/workers/platform/pricing/)
- [Workers 限制](https://developers.cloudflare.com/workers/platform/limits/)
- [D1 定价](https://developers.cloudflare.com/d1/platform/pricing/)
- [D1 限制](https://developers.cloudflare.com/d1/platform/limits/)
- [D1 免费额度开始强制执行](https://developers.cloudflare.com/changelog/post/2026-09-01-d1-free-tier-limit-enforcement/)
- [Durable Objects 定价](https://developers.cloudflare.com/durable-objects/platform/pricing/)
- [Secrets Store 概述](https://developers.cloudflare.com/secrets-store/)
- [Secrets Store 管理限制](https://developers.cloudflare.com/secrets-store/manage-secrets/)
- [Access service tokens](https://developers.cloudflare.com/cloudflare-one/access-controls/service-credentials/service-tokens/)
- [Cloudflare One 账户限制](https://developers.cloudflare.com/cloudflare-one/account-limits/)
- [Cloudflare 套餐页](https://www.cloudflare.com/plans/)
- [2022 Zero Trust 套餐表](https://www.cloudflare.com/static/3fb3993535599c90e3fb6b64f2c11d67/Cloudflare_Zero_Trust_Pricing___Plans__Q4_2022__.pdf)
- [Tunnels for Platforms](https://developers.cloudflare.com/tunnel/features/tunnels-for-platforms/)
- [Containers 定价](https://developers.cloudflare.com/containers/platform/pricing/)
- [容器磁盘 FAQ](https://developers.cloudflare.com/containers/faq/)
- [容器快照](https://developers.cloudflare.com/containers/guides/snapshots/)
