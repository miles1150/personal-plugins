# 改造与验证方案

日期：2026-10-31。仅提案；此文不构成实现、迁移、生产授权或已完成验收记录。

退役更新：用户已明确批准旧独立服务及其未迁移数据直接删除，云端已执行，见 RETIREMENT.md。下文P4/回滚中“旧系统”在后续实施应仅指仍运行的统一服务v1；已删除的独立Waypost/Keys不再作为兼容或回滚目标。历史迁移研究保留供解释，不再执行对已删除数据的迁移。

## 1. 当前差距

| 模块 | 已核实当前实现 | v2 改动 |
|---|---|---|
| 账户/OAuth | `0014_mcp_oauth.sql` family 必填 Space/Actor/Consumer；`oauth_authorization.rs` 批准依赖这些字段 | 并行新增无助手 bootstrap 的 v2 family，不破坏旧协议 |
| Unified MCP | `unified.rs` 以 family/request_id 获取消息/keys grant | v2 每请求解析助手上下文、挂载、助手策略；固定协议 scope |
| 密钥 | `0018_unified_keys.sql` 按 request 保存许可与 ACL，KEYS_DB 独立 | 策略以助手 ID/版本关联；保留原密文、AAD、资源 ID |
| 助手 | 现有 agent Actor 和 inbox Consumer | 显式 Assistant 及 legacy Actor 映射、策略、默认路由 |
| 工作会话 | 旧 thread 是消息对话；bridge config 固定宿主 thread | 新 WorkSession，绝不复用旧 threads 表语义 |
| 路由 | 收件 Actor；无公开会话目录和可空默认分配模型 | 目标意图、会话目录、分配与分配版本 |
| 领取 | `0020_receiver_claims.sql` 主键含 Consumer，最长 300 秒租约，无 renew | 助手投递级 claim + assignment fencing + 续期/未知恢复 |
| Bridge | 已有 PKCE/刷新锁/proof/SQLite/unknown；30 秒活跃、60 秒空闲轮询与抖动 | 安装级 supervisor、多会话适配、独立通知游标、旧账本迁移 |
| UI/插件 | main 统一插件；本地额外账户中英修复尚不能等同线上 | 重做助手中心旅程，更新 skill/tools 描述、真实能力提示 |

准确源码及差异见 `research/auth-migration.md`、`research/bridge-routing.md`。本轮远端 main 95d5830，本地 5cd6944；未运行生产部署、未读生产秘密。

## 2. 推荐部署边界

保持单公开 MCP 入口，Cloudflare Worker 继续提供账户、业务 API、静态资源。控制/消息 DB 和 KEYS_DB 保留隔离，R2 继续存载荷。本机 Rust bridge 保留。第一版不因新模型增加 DO、队列、常驻云端 Agent 或全新认证服务。

读取策略版本、keys DB 策略准备、控制 DB CAS 发布使用现有 fail-closed 模式演进。跨 D1 没有跨库事务：保证撤销完成后开始的新请求拒绝，已授权在途操作可能完成；不宣称绝对撤回秘密。需要更强撤销线性化时才评估串行协调，不把此复杂度默认加入个人项目。

## 3. 数据与 API 草案

新增表名仅示意，开发时以迁移约束审查为准：

| 表 | 关键字段/约束 |
|---|---|
| assistants | account_id, assistant_id PK, legacy_space/actor 唯一映射, status, policy_revision |
| assistant_policies | assistant_id + revision, messages, key_read/write/create, resource_selector；不可出现 mount_id |
| client_installations | account_id, installation_id, host_kind, label, status；不以 hostname/client_id 唯一 |
| client_families | family_id, installation_id, audience, protocol_version, refresh 生命周期、撤销状态 |
| assistant_mounts | mount_id, assistant_id, installation_id, revision, approved_by/at, revoked_at；无业务权限列 |
| mount_requests | 不可变account/installation/family/assistant/policy_revision, expires_at, consumed_at；批准CSRF校验与消费原子 |
| work_sessions | session_id, assistant_id, installation_id, external_session_ref, registration_revision, discoverable, capabilities, state, last_seen |
| wake_bindings | bridge_installation_id, session_id, registration_revision, receiver_proof, revision, status；执行实例与bridge实例分开 |
| assistant_routes | assistant_id, nullable default_session_id, route_revision；同助手 FK/服务层原子校验 |
| message_targets | message_id, recipient_assistant_id, target_mode, requested_session_id；原始目标不改写 |
| delivery_assignments | delivery_id 唯一(消息+收件助手), assigned_session_id, generation, state, reason |
| processing_claims | delivery_id, attempt_id, generation, assignment_generation, holder_session, lease_until, completed_at |
| conversation_execution | conversation_id + recipient_assistant_id, owner_session_id, active_session_id, execution_generation；跨消息领取CAS |
| migration_maps | legacy IDs、v2 IDs、批准状态、版本、统计/校验；不能只存名字 |

工作会话可以多个客户端分别注册；同一外部宿主 thread 的重复注册按实例+宿主+外部 ID 幂等。如果确实没有宿主 ID，则新建手动收件注册，不承诺自动复用。

建议公开工具按使用任务命名：

- `identity`：当前客户端、已批准挂载、可选助手上下文；无挂载时只返回有限初始化信息。
- `assistant_request` / `mount_request`：提交草稿/批准请求，不自行批准。
- `sessions_register/list/update`：在有效挂载范围注册/维护会话；default/discoverable 操作的可自动化边界见决策。
- `recipients_list`：有权联系的助手及其可发现会话，不能列账户密钥或宿主路径。
- `messages_send/read`：显式 assistant 上下文、可选目标 session、对话 ID、幂等键。
- `deliveries_claim/renew/complete`：同时校验 assignment 与 claim generation。
- `keys_list/get/create/update`：唯一策略来自助手，响应不意外泄露值；删除不混入 update。

挂载批准和权限修改走账户网页控制面。工具方法本身可保持稳定，避免依赖每个宿主的动态工具目录刷新。新工具采用 schema 版本，旧 tools 名称和输入在兼容层保留；不能让旧发送接口忽略会话参数并返回假成功。

补充旧工具映射：`waypost_memory_publish/read`（含 delete_ids）建议不在首版 v2 消息勾选下开放，保留旧协议，等待资料能力范围决定；`blob_prepare/commit/get` 在 v2 仅接受具有可访问消息归属的附件，孤立资料上传不得绕过权限。REST、MCP、附件下载、最终 mutation guard 都覆盖新鉴权。已签发下载 URL 的有效期与撤销限制要单独记录。

附件通常先上传后发送：为当前助手建立有时限的待发送附件 reservation，发送消息时原子关联，未关联的过期上传回收。下载仅由可读消息的助手获取；不能因上传早于 message_id 就取消归属校验。

账户内跨旧 Space 的 v2 消息建议采用账户级 `v2_messages(account_id, message_id, sender_assistant_id, conversation_id, payload_ref, request_id)` 和 message_targets，不为每个目标Space伪造同名Actor。新收件读取聚合v2投递与显式legacy映射；旧消息内容/ID/Space不搬动，不自动重放旧历史。旧发送方继续按原Space规则；只有迁入v2后才获得账户内助手目录和跨旧Space寻址。跨账户互发不在首版。此独立lane带来明确兼容成本，但比偷偷放宽旧Space鉴权更可审阅。

会话注册/设置默认/目录展示是有效挂载下的工作配置，允许 Agent 操作并审计；不误设为每次网页批准。权限提升、挂载批准、安全设置仍由账户本人确认。挂载撤销/禁用实例与路由失效必须一并处理；暂停会话保留默认选择并标暂停等待，不自动替补。

## 4. 分阶段实施与退出条件

### P0：先验证接入协议，不先重做全站

隔离原型验证“固定资源+bootstrap凭据→网页批准挂载→同一凭据调用助手工具”。覆盖单安装两助手并发、动态收紧策略、刷新、撤销。Codex/Claude 各运行实际宿主；Grok 先确定实际产品（Web/Build/API），真实验证无法完成时明确降级，不以标准文档当测试。

退出条件：工具调用无需伪造机器/thread；多助手不串身份；宿主无需假定支持 token exchange；账号控制面不泄露给 bootstrap。若需重新 OAuth 才能生效，回到 UX 明确增加步骤，不假装一个页面等于一次授权。

### P1：新增助手权限与挂载基础

添加 v2 schema/鉴权，保持旧路径不变；建立 policy_revision 最终写入校验及跨库 fail-closed；实现身份/助手草稿/挂载批准/策略页面。新挂载不是旧凭据自动升级。

退出条件：授权不再要求 Space/Consumer；助手为唯一业务 ACL；只创建不允许读已有秘密；已批准全部权限修改一次影响所有挂载；未批准客户端不可借名字接管。

### P2：工作会话、公开目录和权威分配

实现会话注册/默认空/软删除/公开目录、原始目标与实际分配分离。先把 claim 和所有旧 receipt/claim 写入口收敛到助手级，再启用多会话，不留旧 Consumer 并发旁路。

退出条件：无默认不丢消息；显式目标不改投；并发删除+发送不悬空；双客户端不重复领取；迟到完成被拒绝；对话跨会话冲突可见而非静默并发。

### P3：bridge 与接收恢复

保留 OAuth/刷新锁/账本，增加安装 supervisor、会话映射、receiver proof v2、claim renew、按分配拉取及独立分页进度。登录预检集中检查可用回调、写权限、宿主版本、目标 thread、有效挂载及身份，不打印 secret。

退出条件：真实 Mac mini 两会话默认/显式投递、重启、离线积压、默认删除、unknown 恢复闭环；不能用模拟 adapter 宣称 Claude/Grok 可唤醒。

### P4：交互整合与非破坏迁移

实现本轮原型与文案，更新插件 skill，复用中英机制但不直接合入旧领域页面。做只读旧连接/Actor/Consumer/账本清点与冲突表；用户批准助手策略和要迁入的客户端，分助手切换。

退出条件：旧身份地址/消息/密钥资源不丢；权限不自动取并集；unknown 不迁为完成；原始目标可追溯；旧收件写入口不旁路新领取。

### P5：Cloudflare 发布与真实验收

预发布隔离 D1/R2、合成密钥；备份恢复验证、schema兼容检查、候选版本配置差异及资产检查；经适用授权发布、先少量助手切换。保留旧 Worker/资源/连接；后续退役独立任务。

上线验收不能止于 /health 和 Google 登录。必须记录消息→实际会话→领取→一次可见业务效果→processed，以及密钥创建/读取/修改/拒绝、刷新/撤销。记录 ID/时间/版本，不记录值/token/code。

## 5. 迁移与回滚的实际边界

1. **映射，不合并名字。** Actor→助手使用固定映射；OIDC issuer+subject不变；owner Actor 不自动包装成业务助手。
2. **旧不同权限不取并集。** 同 Actor 的多个旧 grant 若不同，保留旧访问语义，输出清单；批准助手新策略/挂载后才启用 v2。
3. **旧 Consumer 进度不盲合。** 分清通知、读取、完成；保留历史游标/回执，冲突列待核查。
4. **先读影子路由，再切接收写入。** 新旧 bridge 不同时派送同助手。冻结旧接收自动执行，保存最后账本和未确定任务，再启用新分配。
5. **回滚不是启动旧二进制。** 已有 v2 数据/派送后，旧版本不理解会话目标，必须停新分派、隔离未完成v2记录，确认不会重复，再恢复兼容读写；新 schema 不做 destructive down migration。
6. **旧入口退役有独立门槛。** 用户数据和凭据不随源码清理删除；列每个旧服务/连接最后使用、替代验证与待删除任务。

## 6. 精简验收矩阵

| 层 | 核心用例 | 证据与限制 |
|---|---|---|
| 纯逻辑/SQLite | 空默认、指定目标删除、默认替换、幂等、双claim、fencing、迁移unknown | 有意义的边界测试，不写镜像实现的快照堆积 |
| Worker本地 | bootstrap拒绝、动态助手策略、跨挂载ID拒绝、多助手并发 | 合成数据、官方宿主协议实验；不等于Google实机 |
| 隔离D1 | CAS默认删除/发送/领取竞争、租约renew、策略版本不匹配 | 必须真实D1验证事务语义，不能只用SQLite替代 |
| Mac mini Codex | 两thread注册、显式与默认、无默认积压、删除默认替换、bridge重启、unknown | 真实唤醒+读取+claim+业务效果+回执；用户自行登录 |
| Claude | 真实模型回合工具、同助手权限、会话注册与能力 | 无唤醒适配器则只验pull，不能标wakeable |
| Grok | 明确产品、OAuth/工具/刷新/撤销、主动查收 | 缺设备支持则列未验证；不伪造thread稳定性 |
| 密钥 | read/write/create组合、资源范围、旧token收紧后拒绝、跨库版本冲突 | 仅合成条目；不在日志/交接输出值 |
| 回滚 | 混合旧新身份、账本/unknown保留、切回不重复 | 先隔离演练，生产不试错 |

开发可并行：助手鉴权、会话/路由、bridge适配、UI组件。集成顺序不可倒置：先权威权限/领取边界，再启用多客户端实际执行。

## 7. 初始提案状态（历史）

完成的是源码研究、领域/改造提案、交互原型及独立审阅（具体证据见 README 和 reviews）。未执行 v2 工程实现、生产迁移、真实新协议宿主验收、Mac mini 操作。上述阶段的退出条件都是未来门槛，不是本轮已通过结果。

## 8. 本轮实现与隔离验证补充（2026-10-30 UTC）

- Rust native/SQLite 的会话路由测试覆盖空默认积压和回填、加密载荷、幂等、会话间显式改投冲突、领取和代次、租约到期 unknown、账户指定替补及无默认移除、人工交接代次与审计，以及同助手跨安装读取但不得冒领。
- 在新建的隔离 D1 `d3453086-ec1e-41e5-a498-eed6aa19ce95` 上，以合成数据执行 0022 schema 和触发器，验证触发器回填负责人、两次并发对话 CAS 和投递领取各只有一次成功，以及错误代次回执不更新；验证后该临时库已删除。该证据只覆盖关键 SQL，不能代替完整 Worker、真实宿主和生产部署验收。
- **附件能力的当前边界：** 已实现账户和助手绑定的预留、加密对象上传、消息关联和发件/收件助手读取；native 测试覆盖跨账户拒绝、校验和及加密。首版仅支持受 JSON 请求大小限制的 base64url 传输（当前统一服务配置上限 193,536 字节，本地默认约 45 KiB，以预留响应的 max_inline_bytes 为准），未验收大文件、真实 R2 与 D1 0025；过期回收由任一已授权助手的后续预留触发，空闲期间不会自动物理删除。不得复用独立资料接口绕过助手权限。真实 Claude/Grok 工作回合、Mac bridge 唤醒及完整生产业务验收仍须分别记录。


## 8. 当前工程进展与发布门槛

此节取代以上初始提案中“尚未实现”及“保留旧服务”的时点陈述。用户随后明确授权旧服务及其数据无需备份退役；具体资源记录见 RETIREMENT.md。新统一服务的数据不在删除范围。

当前工作树已实现助手权限、安装挂载、bootstrap OAuth、会话/路由/领取代次、bridge proof 与刷新、原生中英文界面及统一 MCP 接线。插件 0.7.0 为待发布源码版本；已有市场分发不能当作新版本验收。

已通过的验证包括 native 后端集成、独立 bridge 测试、Wasm 构建和本地 dry-run。隔离真实 D1 验证仅覆盖 0022 schema/trigger、并发 CAS/claim 和错误代次拒绝，不是完整 Rust-on-D1 端到端验证。详细数量以最终验收记录为准。

尚未完成：新版本生产部署、真实用户 OAuth 与助手挂载、Mac mini 实际唤醒/业务效果/回执、Claude 模型回合以及 Grok。消息附件和新版共享资料已实现 MCP 核心闭环，但尚未完成真实 R2/完整 Worker 与宿主验收；网页附件控件和独立 vault 管理未实现。

跨 DB 撤销语义：每次调用和敏感写入前检查权威控制库；策略快照只能单调前进。已经通过检查的在途操作仍可能完成，不提供跨两个 D1 的原子即时撤销，也无法收回已返回密钥。
