# Miles Works：接手评审与实施方案

状态：方向与开发、Cloudflare 部署已获用户批准，正在本地实施与验证；生产部署仍在调查，真实设备闭环未验收。最新个人预设采用全部读取、修改与新建，仍须最终摘要明确批准。日期按 Asia/Singapore。

## 接手时证据（历史快照）

- 本地 HEAD 和实时远端 main 都是 `1992d19dd521a1069880c7b49fdb99df2a201e87`；开始时工作区干净。插件 0.4.0，两个 MCP URL。未更改生产代码、现有连接或数据。
- Cloudflare 连接器认证可用；只读列出 `waypost-production` 和 `waypost-keys-production`，最后修改时间分别为 2026-10-29 22:47、22:41（新加坡）。未核验已部署二进制对应的 Git SHA，也未读取数据库或密钥。
- 技术聊天 `01a188fb-a9fc-7600-a3b4-4231af991016`：从完整 fileChange diff 恢复实验及报告；应用了该聊天所有后续代码差异。不是下载原 ZIP，不能声明与 ZIP 逐字节一致。锁文件在本轮按报告中的固定依赖重新生成。
- 本轮重跑：官方 MCP SDK 1.32.2 加 Python 的 10 组边界检查通过；Codex CLI/app-server 0.159.2 的 5 项检查通过；Claude Code 2.1.312 的 4 项检查通过。结果在 experiments/miles-works-unified-mcp-2026-10-29/*-results.json。
- 这是真实宿主进程连接本地合成服务，不是真实 Google、生产服务或模型回合。Claude 只验证发现/健康检查、刷新、撤销；Grok、桌面、cloud 和跨机尚未实测。
- 仓库现有 6 个插件测试、4 个 SQLite 迁移测试通过。当前没有 cargo/rustc；没有声称运行 Rust、Go 或 Workerd 测试。
- 设计聊天 `01a188fb-5f44-7529-aebc-7ebaffc03018` 已完成。从其文件生成记录恢复原型、报告、7 条旅程和系统，重新运行 baoyu 编译器与 checker：44 tokens、12 组件、24 卡片，无检查问题。
- 本轮重跑设计验证：24 项交互检查、40 组页面/视口无横向溢出、8 页面 axe 指定规则无违规；pageerror 与外部请求均为零。自动检查不代替读屏器、Safari 和真实设备。
- baoyu-design 源码固定为 `0fd52fc77765b214cfcc2ced20b092b2969bb0b0`。未改变 Mac 上安装的技能。当前恢复版原型与原 ZIP 的字节一致性未验证。

以上为接手时的证据与限制，保留原时间语义；不代表下面新增实现的当前状态。原技术报告、原设计和历史测试结果不回写。

### 聊天读取范围

已读技术、设计、Mac 验收、接收目标及三个初步复盘聊天的可用记录。长期开发聊天 `01a1811c-d4f7-7578-b720-a6dde639b0e3` 的最新一轮可读；请求多轮返回体积超限，未穷尽早期内容。总体历史 `01a172f6-d704-70a7-98cb-bbde54e1d00e` 返回空 turns，因此只按用户交接及其他可核验证据处理。Mac 聊天可读部分确认技能文件核验与原型路径，但本机磁盘没有挂载。没有猜测其余内容。

## 本地实施更新（尚非生产验收）

### Grok 支持范围补查（2026-10-30）

官方 [Grok Connectors](https://docs.x.ai/grok/connectors) 当前说明可添加公开可达的 Custom MCP URL 并完成认证；[Grok Build MCP](https://docs.x.ai/build/features/mcp-servers) 说明 HTTP MCP 可自动处理 OAuth。不能把 Grok 网页、Build CLI 和 API 当作同一个宿主。

[xAI Remote MCP API](https://docs.x.ai/developers/tools/remote-mcp) 支持 Streaming HTTP/SSE，允许传入 authorization；文档并不因此证明 API 会替调用方完成浏览器登录或刷新。API 的 require_approval 和 connector_id 当前不支持。权限仍由本服务端执行，allowed_tools 只用于缩小模型工具目录。以上仅为官方文档复核；本轮尚无 Grok 真实账户调用、刷新、撤销证据，原 Grok Actor 不能自动迁入新服务。

### 实现和本地验收

- bridge 已增加独立 OAuth PKCE 登录、私有成对令牌原子保存、刷新锁与不确定刷新不重放；保留旧 `token_file`、账本和 unknown 恢复。31 项 bridge 测试与严格 Clippy 已通过。最小通知 scope 尚未实现，当前仍使用 `waypost` 成员能力。
- 本轮代码新增统一 MCP 组合路由与连接授权、接收身份挑战、领取/完成与 fencing；后端、工作台和密钥预设仍以各自本地验证记录为准，不把研究 fixture 或原型点击当作已部署功能。
- 新 OAuth bridge 在真实通知前查询服务端接收证明；receiver 以自己的凭据完成挑战，身份不符不通过。证明反映握手时的凭据一致性，不能实时读取或推断宿主后续配置。旧 token 配置需显式启用此门槛。
- 原型 `proposal.js` 与单文件预览已按最新个人预设调整；仍仅使用合成数据。拟用统一域名、生产 Worker/schema 版本和真实设备状态，必须待部署与实机记录确认。

## 先修正三点认识

1. bridge 已有 `/v0/describe`，会核验自己的 Space、Actor、Consumer 与配置一致（bridge/src/main.rs:315）。缺口是接收线程所用 MCP 的身份没有参与核验，而不是 bridge 完全不核验身份。
2. bridge 已有私有文件、排他锁、SQLite 去重和 unknown 恢复。不能删掉这些后重新做一个简单轮询脚本。通知去重也不等于多个 receiver 不会重复执行业务。
3. 当前 device grant 只允许 source=legacy 的客户端（oauth_authorization.rs:206）。动态注册成功不等于 device flow 可用。优先实现浏览器 PKCE 回调；无浏览器设备另验收固定客户端注册和设备流。

## 推荐产品方向

保留 Miles Works 一个名称、一个插件入口、一个工作台。Waypost 和 Keyenv 作为消息与密钥的内部能力名称；旧安装入口标为兼容，不主动删除。官网讲安装与任务，工作台以收件箱、密钥、助手与连接为主；设备和安全在设置中。Actor、Consumer、family 仅放技术详情。

首次接入：选宿主 → 宿主发起 OAuth → 登录账户并确认清楚的能力 → 回到宿主做一次真实往返。首次准备个人 Space 和普通助手身份；已有连接重登保留稳定身份，不能因刷新/重新登录重新造 Actor。只有身份有歧义时要求选择。新设备默认新连接，可以明确选择复用一个逻辑助手；不默认用 Owner。

消息权限默认只覆盖选定 Space 的本人收发及现有成员共享资料能力，明确保留当前真实语义。账户工作台可以选择账户所属 Space 发信；助手的跨 Space 发送作为独立能力扩展，不能先隐藏选择器就开放。目标默认可发现并发送到已授权账户所属 Space；读取仍限本人消息，资料权限单列。退出成员资格即时影响后续发送。目的 Space 必须具有该账户的合法发送身份映射，首次缺少时走幂等设置流程，不冒充目标 Space 的 Owner。该项依赖新的路由与授权模型。

密钥：初始消息连接不自动获得密钥。按用户最新批准，需要密钥时可一次选择“个人可信助手 · 全部读写与新建”：读取和修改全部当前及未来条目（含生产条目），允许新建；不含账户管理。也可选择只读这次任务的条目集合，或自行调整。预设点击只填选范围，必须在最终摘要明确确认当前/未来、生产条目与写入/新建后批准才生效。历史只读个人预设已被本次决策替代，不根据客户端自报名称判断可信。

现有代码没有密钥分组策略，因此第一版不创造一个没有服务端语义的“安全密钥组”。任务预设明确映射为当前所选条目 ACL，不包含未来条目。创建能力会赋予自己新建条目的读写许可，实际仍与 scope 取交集。读取会把值交给宿主/模型；CLI 注入执行是另一种能力，不能在远程 MCP 返回值后声称模型看不到密钥。

后台接收：在同一设备详情中选择助手和固定接收会话，先由 receiver 使用自己的凭据向服务端完成固定目标挑战，服务端核对 Space、Actor、Consumer，再以合成消息完成读取、持久接收、回执。未完成可验证挑战时保持“待验证”并禁用后台分发，不仅用名称匹配。用户看到“已保存 → 已通知 → 已接收 → 已处理”，附各阶段时间和可恢复错误；处理完成不代表外部业务操作成功。

原设计保留在 `designs/miles-works-review/index.html`；新增推荐方向 `proposal.html`，补充密钥一键预设、默认身份的高级设置和身份不匹配诊断。原型使用虚构数据，拟用 `works.miles1150.top/mcp` 尚未部署。网页工作台与所有者视图不授予普通助手同等读取权。

## 架构选择

| 方案 | 用户体验 | 成本与限制 | 建议 |
| --- | --- | --- | --- |
| 单公开 MCP + 统一账户/授权服务 + 独立业务授权 | 每个宿主一个连接，可在一次 consent 分别选择消息与密钥；以后扩权可能需要宿主重授权 | 需要 connection 模型、principal 路由、后端复核和迁移；一个 token 暴露会影响其已授能力 | 目标方案 |
| 双资源 + 共用登录/授权服务 | 可复用浏览器登录；仍是两份 audience、两份连接，可能两次 consent | 改动较少，作为旧连接兼容；不能承诺“一次登录等于一次授权” | 迁移期保留 |
| gateway 保存两个 owner token 再代调 | 看似单入口 | 扩大权限、丢失业务身份，不能约束密钥访问 | 不采用 |

建议保留现有 Rust 业务实现和通用 MCP transport，增加组合路由，不为统一入口重写消息与加密代码。Cloudflare 上公开 gateway/账户 Worker，消息 Worker 与 keys Worker 通过 Service Bindings 调用；消息 D1/R2 与 keys D1/keyring 保持分离。先做一条垂直切片验证服务边界，再决定是否需拆出单独 AS Worker；不额外引入队列、Durable Objects 或全新框架，除非实测需要。

统一连接关系：OIDC `(issuer,subject)` → Account → logical Agent → Connection（宿主实例）→ message grants / key grants。Space 和 Consumer 是消息业务对象；不能拿邮箱合并账户，不能将 Consumer 当设备名。每台设备/宿主有自己的凭据家族，bridge 也独立；不复制 Codex/Claude 的私密令牌。

gateway 校验 audience、family、scope；业务端复核 account、connection、授权版本及对象 ACL。Service Binding 只解决私有调用，不能替代授权；不接受任意公网身份头，不向旧资源转发新 audience token。keys Worker 从授权服务复核活动连接，再与本地条目授权取交集。敏感写入在本地提交前再次检查相关授权版本。撤销保证拒绝之后开始的新授权，已经获准的在途动作可能完成并留审计；不能承诺跨两个数据库的瞬时原子撤销或撤回已返回密钥。

固定工具目录兼容宿主缓存，调用时逐项鉴权。无有效凭据返回 401；缺 scope 与条目无权区分；scope step-up 未在隔离包验证，不得在 refresh 静默扩大权限。MCP 创建 Space/连接先创建 pending request，幂等返回审批入口；使用浏览器账户权限批准后创建，新宿主再完成自己的 OAuth，不由普通消息工具签发管理令牌。

## 并发与 bridge 的具体合同

- 保留现有本机通知账本、排他锁、stable item_id 和 unknown 状态。启用验证时，在每次消息通知前核验配置、bridge 凭据与接收目标的服务端握手证明；换身份必须显式迁移，旧游标不复用到另一个收件人。
- bridge OAuth credential provider、PKCE、私有存储、跨进程刷新锁与成对令牌原子保存已在本地实现；独立的最小通知能力仍需实现。刷新响应丢失置为“需恢复”，不重放旧 refresh token，不循环弹登录。
- 本轮代码增加服务端领取/完成协议，仍待部署与实机验证：以 `(space, actor, consumer, item_id)` 为唯一工作键，短租约与递增 fencing generation；同一工作流一次仅有一个活动领取者。完成必须匹配当前 generation。收到正文并持久化后才提交 received。
- receiver 的副作用要有稳定业务幂等键。租约过期并不能撤销旧进程已经发起的外部操作；不提供幂等性的操作发生不确定结果时暂停并核对，不能宣称 exactly-once。
- SQLite 本地实验能验证竞争领取与旧 generation 拒绝；需要在 D1 上验证原子条件写与竞争，才启用多设备共同处理。当前 receipts 的幂等与单调阶段不替代此协议。

## 可执行实施顺序

| 阶段 | 交付与关键文件 | 验收门槛 |
| --- | --- | --- |
| 0：收敛方向 | 此方案、proposal.html；恢复实验有固定依赖和结果 | 方向已批准；按最新全部读写/新建个人预设实施，不把旧报告当新批准 |
| 1：可靠接收切片 | bridge credential provider/receiver handshake；mcp identity；claim/complete 迁移与唯一约束；沿用现有回执 | 两个 receiver 竞争仅一方领取；旧 generation 完成被拒；重启不重复通知；身份不符停止；未知结果不重发 |
| 2：统一连接切片 | gateway/mcp.rs、service/mod.rs、oauth*、keys/tools.rs；新增 connection 与 grants 迁移 | 单连接两类工具；消息不能读 keys；读不能写；scope/ACL 取交集；撤销/刷新；新旧 audience 不能混用 |
| 3：完整 UX | account、keys、dashboard、website 共用 tokens；默认身份事务；权限预设；doctor；更新 miles-works manifest | 全新安装和已有安装均无需输入 Actor/Consumer；错误有可操作恢复；批准范围准确；旧连接仍可用 |
| 4：Cloudflare 上线与实机 | 独立 staging Worker/D1/R2/keyring；Google 测试回调；生产构建、迁移、版本记录与回滚 | staging 真实宿主闭环通过，再部署生产；记录 Git SHA、Worker version、schema；旧连接验收后逐个迁移 |

测试控制在关键授权、迁移和并发边界，复用已有 suites；不为文案与每个按钮堆集成测试。每个切片附少量可复跑测试和一条真机任务。本轮已补齐隔离 Rust 工具链；Worker 生产构建、迁移与部署必须分别留存结果，不能拿 Python fixture 替代。

## 安装、授权与部署预检

`works doctor` 目标输出结构化但不含凭据的检查：宿主版本/能力、插件重复入口、MCP metadata/resource、网络/回调、当前服务身份、receiver 身份、refresh 到期状态、bridge journal/锁/进程和最新各阶段证据。失败分类为网络、缺授权、身份不匹配、未运行、投递未知；给一条明确下一步。

部署预检一次收集：目标账户/Worker/域名、当前版本与绑定名称、OIDC 回调、凭据是否存在（不读值）、构建环境、迁移备份、回滚点和测试设备。现有 Cloudflare 连接器可用，不先索取 API token。真实 Google 同意、设备安装与持久凭据授权集中在实机窗口提出；不让用户手工转抄 code/token。代码可自主完成的部分先完成。

生产迁移采用新增表/兼容读取，保留原 account/actor/consumer/message/request_id/ciphertext AAD。不能按同名或同邮箱自动合并身份。新旧 OAuth 家族独立，旧 token 只能用于原 audience；不通过 URL 重定向“迁移 token”。先添加新连接、验证消息与合成 key，再让用户撤销旧连接。旧仓库、marketplace 条目和连接先保留，按下面清单验证后逐项迁移或删除，不因统一命名批量移除。回滚切回旧 Worker 版本并保留新增兼容表，先验证旧版本能读取；不靠破坏性 down migration 追回新写入。

## 旧入口与连接迁移／删除待办

| 对象 | 执行人／批准人 | 先做与验证门槛 | 才可执行的变更 |
| --- | --- | --- | --- |
| 旧 Waypost／Keyenv 插件与 marketplace 条目 | 实施方整理清单；账户所有者批准具体条目 | 记录来源、版本、使用设备与回滚安装方式；统一 Miles Works 入口在各现用宿主完成消息及合成 key 验收 | 先标兼容／弃用，再按所有者明确批准移除旧入口；原源码与历史证据保留 |
| 旧 MCP URL 与消息／密钥 OAuth 连接 | 实施方列脱敏 ID 与用途；所有者逐连接批准撤销 | 新连接完成对应工具、强制刷新与撤销测试；保持 Actor/Consumer 和原 audience；旧连接无未完成任务 | 先撤销被替代凭据并验证后续拒绝，再删除宿主的旧配置；不重定向或复制旧 token |
| 旧 bridge、启动项与固定接收会话 | 实施方迁移；设备所有者批准切换／卸载 | 备份私有账本；核对 pending/unknown；新 OAuth 与 receiver 挑战通过；重启、重复 feed 不重复通知 | 停用旧实例后启用新实例；明确批准后卸载旧启动项；不删除未核对账本或并行唤醒同一任务 |
| 旧 Worker、域名与 Service Binding | 实施方记录版本／绑定；服务所有者批准退役 | 新路由回归、真实设备与回滚演练通过；确认无剩余客户端或回调引用 | 先停止推荐旧地址，保留回退窗口；按所有者批准逐一退役；生产调查未完成前不删 |
| D1／R2、密钥条目与账户身份 | 实施方只做兼容迁移；数据所有者单独批准删除 | 备份可恢复；验证 message/request ID、AAD、授权边界与审计；给出具体对象、保留期和影响 | 新增兼容表不等于准许清库；禁止按同名／邮箱合并或顺带删除数据，破坏性删除须对象级批准 |

现有开发与部署授权覆盖实现、构建和获准部署；不等于批准上述具体的删除、合并、旧连接撤销或生产数据清理。每项记录执行者、批准范围、验证证据和回滚点；未过门槛保持待办。

## 真实设备验收清单

| 场景 | 操作与通过证据 | 当前状态 |
| --- | --- | --- |
| 新 Mac + Codex | 安装 → Google → 宿主 identity → 发/读消息 → received/processed；记录版本与耗时 | 未验证 |
| 同 Mac + Claude | 单 URL 登录并在模型回合真正调用消息与合成 key；强制过期刷新、撤销失败 | 协议隔离通过；模型/IdP 未验证 |
| Codex cloud | 新会话复用授权是否成立；重建环境后身份稳定；消息与 key scope 分离 | 未验证 |
| Grok 实际产品 | 先识别 Web / Build / API；入口、OAuth、调用、刷新、撤销分别记录 | 未验证；不以文档替代 |
| bridge 与 receiver | 初始不匹配必须阻止；正确绑定后唤醒、读取、持久记录、回执、一次回复 | 本地挑战与 gate 测试通过；本轮未连接真实设备 |
| 并发/掉线 | 双 receiver、重启、重复 feed、刷新响应丢失、租约过期/旧 worker 恢复 | 本地协议与测试已增加；D1 竞争及设备待验收 |
| 密钥预设 | 只消息拒绝；任务集合只读；个人预设全读／全写／新建含未来与生产条目；最终确认前零授予；缩小范围与单独撤销不影响消息 | 原型已更新；本地权限实现需结合 scope/ACL 验证；生产未验收 |
| 迁移与回滚 | 旧地址正常、新地址新授权、稳定 Actor/Consumer、旧账本保留、回滚后旧连接可用 | 未验证 |

每台设备另记录宿主／插件／bridge 版本、Worker version、schema、脱敏 connection ID、测试时间与 message_id/request_id；确认拒绝用例与回滚也有证据，不记录密钥值。个人全量预设用合成条目验证新建后默认可读写、旧条目可修改、未来条目可读写，并验证最终批准前调用被拒绝。

截至此更新，尚未记录可用的 dot／本机控制与 Mac VPN/TCP 验收通道，不能声称已完成实机操作。若后续工具开放，按以上固定清单让 dot 操作；否则集中交付一段脱敏采证命令和必要浏览器步骤。端到端完成以双方的 message_id、身份、received/processed、回复证据为准，不以“部署成功”“已登录”或通知 202 代替。

## 本轮已接受方向与未过门槛

已接受：以单 MCP 为目标；旧双资源兼容迁移；保留可靠接收账本；个人密钥预设一键全读／全写／新建，最终摘要明确批准；完成开发、Cloudflare 部署和可用设备实机验收。旧入口、连接或数据的具体撤销／删除按上表另行批准。真实 IdP、Grok、自动 step-up、D1 竞争和旧数据迁移仍为明确门槛。

参考：[Cloudflare Service Bindings](https://developers.cloudflare.com/workers/runtime-apis/bindings/service-bindings/)、[D1 binding API](https://developers.cloudflare.com/d1/worker-api/d1-database/)、[baoyu-design](https://github.com/JimLiu/baoyu-design)。Cloudflare 文档已通过连接器检索；其余宿主文档链接和上轮源码依据见恢复的技术报告，不能算本轮实机验证。
