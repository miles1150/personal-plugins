# miles-works 技术可行性与隔离验证

核验时间：2026-10-29 18:54 UTC。基准：`1992d19dd521a1069880c7b49fdb99df2a201e87`。本机初始分支名为 `work`，不是 `main`，但 `git ls-remote origin refs/heads/main` 的结果与本机 HEAD 完全相同；研究开始、结束前分别核验。未读取生产数据库、真实密钥、真实宿主凭证或 `.dev.vars`；未调用生产 Waypost/keys、合并、推送或部署。文中“建议”均为下一阶段设计，不代表现有实现。

## 结论

**建议目标是一个 miles-works 插件、一个公开 MCP URL、一个账户入口，每个宿主安装独立连接；消息与密钥保留独立授权和后台存储边界。** 实验已经在真实 Codex 上验证一次 OAuth/DCR 登录后，同一 MCP 资源发现和调用两类工具，刷新及撤销均生效。Claude Code 同样完成真实 OAuth/DCR、发现工具、刷新及撤销检查；未执行 Claude 模型回合中的工具调用。

这证明接入机制可行，不代表现有 Waypost 与 keys 已合并。当前代码的关键缺口是：两种 Principal、两种授权家族、产品模式互斥路由、浏览器会话分别属于各自 origin，以及 bridge 不具备 OAuth 生命周期管理。不能仅修改插件 JSON、共享 cookie 或复用同一 bearer 来完成。

## 已验证的代码事实

以下路径均相对仓库根目录，行号对应上述提交；不是把历史设计报告当作现状。

| 事实 | 代码证据 | 影响 |
|---|---|---|
| `miles-works` 版本 0.4.0，包含 `waypost` 与 `keyenv-remote` 两个 remote MCP 配置 | `plugins/miles-works/{plugin.json,mcp.json}` | 一个插件目前仍是两个资源连接 |
| MCP 协议层已有通用 `ToolCatalog`；先认证再处理 JSON-RPC；catalog 的 `tools()` 不接收 Principal；`listChanged=false` | `services/waypost/src/gateway/mcp.rs:26,45,123` | 可复用协议层；如果按授权动态裁剪列表，需要改接口及缓存行为；固定工具列表、调用时鉴权更容易兼容 |
| `App::handle` 检查 `keys.is_some()` 后直接进入 `keys_handle`；keys 模式没有 Waypost 路由 | `services/waypost/src/service/mod.rs:229`，`keys/mod.rs:47` | 同一个现有 App 实例不能直接同时提供两套产品 |
| resource 为 `public_origin + /mcp`；Waypost scope 为 `waypost`，keys 为 `keys:read keys:write`；authorization 与 token 均校验 resource | `oauth.rs:183`，`oauth_authorization.rs:82`，`oauth_tokens.rs:85` | 两个域名的 token 不可互换；两个资源的 scope 也不能直接相加 |
| Waypost 授权家族绑定 account/Space/Actor/Consumer；keys 家族绑定 account、consent、条目授权；keys 另有 KeysPrincipal | `oauth_authorization.rs:387`，`oauth_tokens.rs:50,193`，`keys/tools.rs:12,227` | 统一连接应成为两个业务授权的父对象，不应强行把个人密钥放进 Space 成员权限 |
| 账户身份来自已验证 OIDC 的 `(issuer, subject)`，而不是邮箱 | `accounts_oidc.rs:397`，`accounts.rs:281` | 迁移需显式映射内部 account_id；同邮箱、同 Google 账户不表示两套数据库已有共同内部身份 |
| HTTPS 使用 `__Host-waypost_account`、HttpOnly、SameSite=Lax、无 Domain；会话保存在部署自己的数据库 | `accounts.rs:44,155,298` | 当前浏览器会话不跨子域；不应降级成父域共享 cookie 来追求一次登录 |
| 账户页面支持创建个人 Space、创建 agent、选择 Space；创建 Space 在无既有 account_spaces 时执行；OAuth consent 要求已选 Space 和明确 Actor/Consumer | `accounts.rs:454,527,602`，`oauth_authorization.rs:420` | 不能描述成“登录自动创建并自动绑定好一切”；这些步骤可以在新统一 onboarding 中合并呈现 |
| Waypost 现有普通连接是 Space 成员能力；共享 collections 的 describe 返回可读可写；消息仍按接收人和会话规则过滤；无细分只读 scope | `service/mod.rs:425,466,513`，`service/privacy.rs`，`service/memory.rs` | “我的设备都能加入个人 Space”可作便利默认，但不能说普通连接仅收通知或可以读所有私信 |
| keys 要求 account sign-in、owner subject、DCR 开启、operator 关闭；并具有按条目的读/写、create、all_read、all_write 授权 | `keys/mod.rs:29,160`，`keys/tools.rs:299` | 当前已有实质授权边界，应保留；`all_read` 与“所选既有条目”对未来新增条目的含义不同 |
| keys `fetch_entry` 返回明文值；`fields` 只是返回字段筛选，条目 ACL 不是字段级授权；写操作不回显值 | `keys/tools.rs:339,430,500` | 不能承诺密钥不会进入宿主/模型上下文，也不能把调用参数中的字段筛选当作字段级安全控制 |
| keys 使用服务端持有的 AES-256-GCM keyring，AAD 绑定账户/条目/字段/key id | `keys/crypto.rs:1,64` | 静态存储加密，不是服务端不可解密的零知识保管库 |
| 两产品都有刷新轮转、重放撤销、家族撤销路径；keys 每次操作检查有效授权，写入及 fetch 审计还会复核 | `oauth_tokens.rs:193,273,312,433`，`keys/tools.rs:258,389,400` | 统一入口应沿用即时失效原则，不能仅在登录时计算一次长期有效权限 |
| Rust bridge 请求前读取私有 token 文件，校验 describe 身份；没有登录/refresh 实现 | `bridge/src/main.rs:283,315` | 给它现有宿主 access token 并不能解决后台续期 |
| 人用 keyenv-remote CLI 已有 DCR、私有凭证存储、带锁刷新；公开命令仅 login/logout/set/import，没有 remote run/get | `tools/keyenv/internal/remote/client.go:112,207,376`，`cmd/keyenv-remote/main.go:30` | 刷新管理可以借鉴；“远端密钥直接注入任意本地命令”目前不是现成功能 |

官方协议核验采用 [MCP 2026-07-28 authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)：resource/audience 绑定、PRM 发现、PKCE、issuer 校验及刷新安全都应保留。该版本优先支持 CIMD，DCR 已标为兼容机制，不能把 DCR 说成所有新宿主都必须支持。当前项目协议层仅列出 2025-03-26/06-18/11-25 三个版本；本实验协商 2025-11-25，没有验证完整 2026 协议实现。禁止把入口收到的 token 原样传给不同上游资源，见 [MCP audience 与 token passthrough 规则](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#token-audience-binding-and-validation)。

## 三种架构的取舍

| 方案 | 用户看到什么 | 安全/工程成本 | 判断 |
|---|---|---|---|
| A：单公开 MCP，统一 AS，业务授权分离 | 每个宿主配置一个 URL、完成一次连接授权；可一次选择消息和指定密钥 | 需要新连接模型、统一 principal dispatcher、授权与撤销联动；一个允许读密钥的入口 token 被盗时，影响其全部已授权工具 | 推荐目标。实验已证明协议与至少 Codex 实际工具调用可行 |
| B：两个资源共用 AS 和账户登录 | 可以复用浏览器 SSO，不必重复输入身份；仍有两个 MCP 配置、两份资源 token/连接授权 | 改动较小，业务隔离清晰；公共 AS 也必须按 resource 签发 token；第二次 consent 是否可减少取决于政策和宿主 | 推荐迁移中间态或明确需要物理隔离的回退；不能承诺“一次连接” |
| C：新 gateway 持有两个服务的长期 owner token，代用户调用 | 表面是一个 URL | gateway 获得跨业务超级权限，入口授权无法约束上游；审计身份丢失，密钥放大风险 | 不采用 |

A 不要求合并消息数据库与密钥 keyring。建议新 gateway 负责公共 MCP/OAuth，keys backend 独立保存密文与数据密钥。内部调用使用受限服务绑定/独立 audience 的短期凭证，传递可核验 account_id、connection_id、grant revision 和具体动作；backend 自行复核 keys grant。不得接受公网调用者伪造的身份头，不能通过“这个请求来自 gateway”就跳过条目 ACL。服务绑定/凭证交换是建议，当前实验未实现该后台部署形态。

**统一的是人看到的账户和连接，不是跨宿主复制 token。** Mac、另一台电脑、Codex cloud、Claude 各自持有独立家族和撤销记录。浏览器仍可复用 IdP 会话；每个新宿主仍需建立可撤销的委托。云宿主是否在新会话复用连接，是宿主账户层能力，不能由本插件保证。

## 建议的权限与默认行为

身份关系应为：`Account → logical Agent → Connection(host installation/device) → Grant(s)`。Space 是消息业务容器；Consumer 是处理流，不是设备别名。连接名称可叫“Mac mini · Codex”，但权限只绑定稳定 ID，DCR 的 client_name 只能显示为客户端自报名称，不能据此认定可信宿主。

首次用浏览器登录时，创建或复用一个个人 Space；新宿主默认建立普通 agent 的独立连接，可选择复用已有逻辑 Agent。不要默认选 Owner Actor。消息默认发给明确配置的主助手；默认接收人只影响路由，不扩大访问权。不同独立处理工作流用不同 Consumer；同一工作流的冗余设备可有意识地共享 Consumer。否则一台设备确认输入，会干扰另一台设备的未处理语义。

初始连接默认只有消息能力，keys ACL 为空；首次连接页允许用户明确勾选指定条目的读/写，完成一次组合 consent。用户若选择不授予 keys，后续需要 keys scope 时应进行有界 step-up，不能在 refresh 时静默扩权。已有 `keys:read` scope 内新增所选条目可以通过 owner 管理页修改 ACL，服务端每次调用复查；无需让用户重复搬运值。`all_read/all_write` 是明确的长期扩大授权选项，不作默认。不要把 keys 的账户私有存储默认共享给整个 Space。

Scope 与条目 ACL 必须取交集：`有效家族 ∧ 账户有效 ∧ scope允许 ∧ 业务对象ACL ∧ 授权版本有效`。撤销整个连接同时使消息和密钥失效；单独撤销 keys grant 保留消息连接。对手工直调 tools/call 同样鉴权，隐藏工具和 skill 文案不构成权限控制。目录可固定包含两类工具，避免授权后宿主 catalog 缓存导致体验不一致；缺权限调用返回可操作的安全错误和授权入口。

现有网关把业务错误放入 HTTP 200 的 MCP `isError`，并不会发 `403 insufficient_scope` challenge。因此下一阶段必须明确区分：token 失效用 401；可 step-up 的 scope 缺失用协议支持的 403 challenge；对象 ACL 拒绝用安全业务错误。**当前实验只验证后两类工具业务边界，没有验证宿主自动 step-up。** 某些宿主只提示重新授权，需要提供清晰入口，不能反复弹授权。

MCP 可以创建 Space/连接的“请求”，但普通 agent 不能由此取得账户管理权限。建议提供 `works_setup_request` / `connection_request`：幂等创建 pending 操作，返回 owner 确认页面；浏览器用自己的账户会话和 CSRF 做审批，服务端复核 Space 所有权。新连接完成自己的 OAuth 后绑定该申请，申请工具不返回新 bearer。实验已验证 proposal 不签发凭证；真实创建/审批事务未实现。

减少人工处理密钥应分两条路径：云端 MCP 只能在用户授权后 fetch，值会交给宿主；本地需要“注入执行而不输出值”时，沿用本地 keyenv run 或另行实现受控 remote-run 客户端。不能用一个远程 MCP 返回“已设置本机环境变量”来假装完成注入，也不建议为此暴露任意远程 shell。`keyenv-remote logout` 目前只删本地文件，明确提示去网页撤销；统一 CLI 应提供“退出本机”和“撤销该连接”两个含义清晰的动作。

## bridge：保留的可靠性与需要补的生命周期

当前实现有实际可靠性机制：私有目录、进程排他锁、SQLite FULL 同步、item_id 去重、服务/Space/Actor/Consumer/宿主线程绑定；先持久化游标和条目，再通知宿主。`sending` 中断转为 `unknown`，未知结果不自动重发；reconcile 只接受目标线程 user message 中的 marker 作为证据。见 `bridge/src/main.rs:209,327,422` 与 `bridge/src/codex.rs:26,149`。这些是代码核验，未在本环境运行 Rust bridge 测试。

Waypost 的发送幂等键是 `(Space, producer Actor, operation, request_id)`，并比较 fingerprint；应保留 Actor 与 request_id 穿过 refresh/迁移，不要按新 token 生成新幂等键。见 `service/mailbox.rs:29,88,206`。回执绑定 Consumer，阶段单调推进；见 `service/receipts.rs:25`。宿主 queue accepted、输入 durable received、业务 processed 是三件事。当前 bridge 不自动发送 received/processed 回执，不能把队列受理包装成处理完成。

建议把 bridge 收到用户设置中的“设备连接”下，但进程可以继续独立运行。增加 `works login --profile bridge` 或系统凭证代理：只拿通知必要能力、无 keys grant；使用自己的 DCR/CIMD 注册、PKCE，按已验证身份绑定连接。现有 Waypost 没有通知只读 scope，需要先增能力控制，不能把现有成员 token 描述成最小权限。不要直接抓取 Codex/Claude 私有凭证文件来共用 refresh token。

刷新必须单写者/跨进程锁、保存新 token 对后再使用、文件 0600/目录 0700、原子替换。refresh token 已使用但响应丢失属于不确定结果，不能无脑重试导致重放撤销；保留恢复状态，必要时提示重新授权。遇到身份变化、撤销、invalid_grant 时停止分发；瞬时网络错误退避，认证错误不循环弹浏览器。保留旧 state_dir 和 item_id journal，切换 credential provider 不等于建立新通知历史。

## 宿主支持：实际结果与未验证边界

| 宿主 | 文档/源码核验 | 本次真实隔离测试 | 仍需实机 |
|---|---|---|---|
| Codex CLI/app-server 0.159.2 | CLI 实际提供 `mcp login --oauth-client-registration auto/cimd/dcr`、`--scopes`；官方源码自动模式优先可用 CIMD、回退 DCR | DCR、PKCE、callback、保存凭证；一个 MCP 发现两类工具；实际调用读密钥、发消息；写密钥被拒；服务端令牌过期触发自动刷新；家族撤销后调用失败 | Codex 桌面/cloud 各自的凭证复用、真实 IdP、真实生产部署。未发任何模型请求 |
| Claude Code 2.1.312 | 官方支持 OAuth、DCR/CIMD、固定 callback、手动 client_id、scope pinning、自动刷新 | CLI DCR/PKCE 登录、保存凭证后 `mcp get`；服务端观察 initialize 与 tools/list；强制 access 过期后观察 refresh；家族撤销后显示未连接 | 没有 Claude 模型回合内 tools/call；Claude Desktop/claude.ai、真实 Google/云账户未验证 |
| Grok Build | 官方文档有远程 HTTP OAuth、MCP CLI 与 credentials 文件；skills 为工作流指令 | 未运行 Grok | 在独立 profile 验证 DCR/CIMD、refresh、revoke；不能只凭“自动 OAuth”推断具体兼容性 |
| Grok web | 官方有 custom MCP connector，要求公网可达 | 未访问 Grok 账户 | 用户账户是否具备该 UI、OAuth 细节、刷新/撤销均未验证；loopback fixture 不可直接供其连接 |
| xAI API | Remote MCP 支持 authorization/headers、allowed_tools | 未调用 API，无 API key | 这是调用方提供 token 的接口；不能推断其代办 OAuth/DCR/刷新 |

[Codex 官方源码](https://github.com/openai/codex/blob/184c9c555ec7b30bacde5a5bdfa92ff156315aa3/codex-rs/rmcp-client/src/oauth_client_registration.rs) 是另行拉取的上游快照，不保证与环境 binary 构建提交相同；运行结果以已记录 binary 版本为准。[Claude 官方 MCP 文档](https://code.claude.com/docs/en/mcp) 支持表中的配置/刷新说明；未把公开 issue 当作现状证据。一个实际发现：本 fixture 中 Claude login 请求了 metadata 中的全部三种 scope，而 Codex 按显式传入 scope 请求两种；因此服务端 consent 与 ACL 不能依赖客户端“应该只申请最少权限”。

Grok 本阶段只考虑 MCP、skill、CLI 三种接入面，不新增唤醒/webhook 方案。[Grok Build MCP 文档](https://docs.x.ai/build/features/mcp-servers) 描述 `grok mcp add --transport http`、首次 OAuth 和 `doctor`；[skill 文档](https://docs.x.ai/build/features/skills-plugins-marketplaces) 描述 skills 路径，且 `allowed-tools` 不授予权限。[Grok web custom connector](https://docs.x.ai/grok/connectors#custom-mcp-connectors) 可连接公开服务器。[xAI API remote MCP](https://docs.x.ai/developers/tools/remote-mcp) 的 token/headers 是服务端调用输入；该文档还说明 `require_approval` 不支持，所以最终授权必须由 miles-works 服务端执行。CLI 只适用于实际有 shell 的 Build 等宿主；web 上一份 skill 不会凭空获得本地 CLI 执行能力。协议兼容不等于插件包装格式已验证兼容。

## 交付物、执行命令和结果

实验位于 [`experiments/miles-works-unified-mcp-2026-10-29/`](../experiments/miles-works-unified-mcp-2026-10-29/README.md)。包含 loopback-only Python 服务、边界测试、真实宿主驱动、官方 TypeScript SDK 测试、锁定 npm 依赖和三份无敏感值结果 JSON。服务只在 `127.0.0.1` 随机端口启动；数据/账户/密钥全部合成。真实宿主使用新建 0700 profile，运行后删除凭证目录；无模型回合。

实际命令：

```sh
cd experiments/miles-works-unified-mcp-2026-10-29
npm ci --cache /tmp/works-lab-npm --no-audit --no-fund
python3 verify.py --sdk
python3 host_verify.py codex
python3 host_verify.py claude
```

结果：`boundary-results.json` 10 组通过；`codex-results.json` 5 项通过；`claude-results.json` 4 项通过，`refresh_observed=true`，`revoked_health_connected=false`。不把测试组数等同于全面安全审计；服务没有真实 IdP/consent UI、TLS、持久化、速率限制或完整 OAuth 边界实现，禁止部署这个 fixture。组合 principal、backend exchange、真实 keys 加密以及生产 migration 仍需实现。

另在当前仓库实际运行：

```sh
python3 -m unittest discover -s services/waypost/tests -p 'test_account_plugin.py' -v
python3 -m unittest discover -s services/waypost/tests -p 'test_*migration.py' -v
```

6 个插件测试、4 个 SQLite migration 测试通过。Rust/cargo 不在当前环境 PATH，未运行 native 服务及 Rust suites；`/usr/bin/go` 实际不是 Go toolchain，执行 `go test ./internal/remote` 返回 `Go: Unknown option: test`，因此不能声称 Go client tests 已通过。这是工具链限制，不是代码测试失败。没有为此访问生产或降低网络/凭证限制。

## 迁移顺序与下一阶段 TODO

1. **先做本地 schema/API 切片。** 新增 connection 父对象与独立 message_grants/key_grants；稳定映射旧 account/Actor/Consumer/family，不改变消息 IDs、request IDs、旧密文 AAD。产出 migration+回滚快照测试。
2. **实现单资源 dispatcher。** 复用通用 MCP transport，拆除 `keys.is_some()` 对路由和 scope 的全局开关；两业务保留自己的授权 adapter。增加一条组合连接的集成测试以及跨业务直调拒绝、撤销竞态测试。
3. **统一账户/AS，保留旧资源。** 两资源先指向同一账户登录与授权服务器，旧 token 只被原 audience 接受；不能靠 redirect 或 host alias 让旧 token 在新 URL 生效。引入新的规范 MCP URL，用户只需一次明确重连；兼容窗口中独立展示旧连接并可撤销。
4. **完成 onboarding 与权限扩展。** 浏览器一次创建/复用个人 Space、选择/创建 agent、连接命名、指定 keys；MCP 创建申请不签发管理凭证；实现 scope step-up 与单独撤销 keys grant。明确 `all_*` 包含未来条目。
5. **bridge 凭证升级。** 新 credential provider、带锁刷新、最小通知授权、撤销后停止；保留既有 journal，失联/未知投递明确可恢复。后台安装仍是用户设备上的独立工程任务。
6. **更新插件与宿主验收。** 只在隔离环境验证新单 URL 后改 manifest/skills 文案；新命名保留旧工具兼容期，内部名字不直接要求用户理解。提交评审材料；本任务不部署或合并。

实机验收控制在以下 8 个场景，全部使用测试账户/模拟密钥：① Mac Codex 与 Claude 各一次连接并发现两类工具；② 新 cloud session 的连接复用实测；③ 只消息/指定条目读/写/创建/全部未来条目分别验证；④ 正常过期刷新、两进程刷新与响应丢失恢复；⑤ 撤销单 keys grant 保留消息、撤销整个设备两业务均停；⑥ 新 Space/连接的 owner 审批及恶意越权参数；⑦ bridge 重启、重复 feed、投递 accepted 后响应丢失、Consumer 切换；⑧ Grok Build/web 用实际支持的接入面登录/刷新/撤销并记录版本与结果。不能验证的项目逐项保留未验证状态，不用协议支持替代实机通过。

当前无待批准的生产动作。剩余限制：真实 IdP、桌面/cloud/Grok profile、bridge 进程及 backend 服务绑定未做端到端实机验证；这些需要下一阶段的隔离部署与测试账户。无需接触真实密钥即可完成。
