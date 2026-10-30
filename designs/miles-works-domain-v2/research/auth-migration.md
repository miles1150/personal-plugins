# 授权与迁移研究：助手作为唯一业务权限主体

日期：2026-10-31（Asia/Singapore）。本文件是设计研究，不是生产实现或端到端验证。

## 核验范围

远端 main 经主任务核验为 `95d5830253923bdac6afe8cb147ce1fc21f3dcc1`；本地研究 HEAD 为 `5cd69447a61f4ec002b1aab5df9d79c0c7089889`。`git diff --stat 95d5830..5cd6944` 显示主要为账户 UI、国际化、测试及 accounts.rs 的首次设置逻辑；本文引用的 OAuth、unified、keys 核心授权与迁移文件未在该差异中改变。没有读取生产数据库、token 或密钥。

## 当前实现证据

| 事实 | 源码位置 | 对改造的意义 |
| --- | --- | --- |
| OAuth family 必填 account、space、actor、consumer；access token 复制后三者 | `services/waypost/migrations/0014_mcp_oauth.sql`；`src/service/oauth_tokens.rs:321-325` | 当前无法直接签发“已登录、尚无助手”的同类 token；不能仅隐藏选择器 |
| OAuth 批准要求 actor 和 consumer，同步核实账户 Space 成员资格及 inbox 关联 | `src/service/oauth_authorization.rs:447-492` | 必须拆分接入与助手挂载流程 |
| Unified 业务权限按授权请求保存 messages_enabled、keys_enabled 和 key_version | `migrations/0017_unified_connections.sql` | 这正是用户决定取消的新模型中的连接级业务权限 |
| 密钥 grant 和逐项 ACL 以 request_id 为键，保存 all_read/all_write/can_create；与 keys DB 分开存储 | `migrations/0018_unified_keys.sql` | 权限应迁到 assistant_id，不能把多个旧 grant 简单取并集 |
| 每次统一 MCP 调用重新读取父 family，消息工具再走 Waypost principal，密钥工具检查单独 DB 中的 grant version 和 issuer/subject | `src/service/unified.rs:29-60,135-176` | 可沿用 fail-closed、跨 DB 版本核对模式；不能只在 token 签发时检查 |
| 普通业务 mutation guard 带 token/actor/consumer 版本和撤销检查 | `src/service/mod.rs:446-548` | 改造必须覆盖最终写入 guard，而不只是入口鉴权 |
| 身份匹配使用 OIDC issuer+subject，不能按显示名或邮箱合并 | `src/service/unified.rs:176` | 旧新账户桥接保持原规则 |
| MCP identity 目前返回 account、family、space、actor、consumer 和连接权限 | `src/service/unified.rs:42` | 新 identity 应返回当前助手、挂载客户端、助手权限及会话上下文；旧返回结构保留兼容版本 |

路径中的 `src/...` 与 `migrations/...` 均相对于 `services/waypost/`。

## 已接受的业务约束

- 助手是唯一业务权限主体，权限复选框：消息、密钥读取、密钥修改、密钥新建。客户端挂载没有第二份业务权限。
- 一个助手可挂载多个客户端并有多个工作会话；thread 之间不隔离权限。
- 助手权限不同仍是实际安全边界；不能因为 thread 不隔离而使账户所有助手可被任意冒用。
- 工作会话、默认路由、收件人展示不授予权限。注册公开会话不意味着公开账户密钥或向任意账户开放收件。
- OAuth family、刷新 token、撤销、审计保留为技术生命周期记录，不再是权限编辑实体。

## 建议的授权架构

### 1. 账户接入与业务使用分阶段

新增 V2 credential/family，而非破坏旧表的 NOT NULL 约束。

`client_installations(installation_id, account_id, host_kind, label, credential_generation, status, created_at)` 记录一次安装或托管客户端注册。物理机器、client_id、hostname 均不是其主键。通用远程 MCP 无法提供可靠硬件身份时，显示“客户端连接/运行环境未识别”，不伪造 Mac mini 归属。

`client_families(family_id, installation_id, oauth_client_id, resource, protocol_version, expires_at, revoked_at)` 记录协议凭据，只承载有效性与挂载关系，不承载 read/write 等业务许可。

首次 OAuth 登录返回的 V2 客户端凭据只允许有限初始化：查看自己的身份、提交创建助手草稿或挂载申请、轮询申请结果。不能列出所有消息/密钥，不可自行批准权限或挂载。网页账户会话仍是管理操作的审批主体；它不是“另一个拥有所有业务权限的助手”。

创建助手可在同一步为用户展示权限复选框及资源范围，然后批准挂载。空助手可以由 Agent 提交草稿，带权限的新助手或挂载已有助手由用户批准；否则任意新接入客户端可以通过“挂载已有全权限助手”绕过授权。

### 2. OAuth scope 与业务权限避免两套真相

推荐 V2 使用固定、明确命名和说明的协议 scope（例如 `works:client`，最终名字待协议实验），表达“代表用户后续明确挂载的助手访问服务”。read/write/create 权限只来自助手政策，不继续把它们复制到 connection/family。

OAuth 首次确认必须准确说明：当前尚无业务访问；后续在 Miles Works 批准挂载后，客户端可以使用所挂载助手的权限，助手权限修改会影响其所有已挂载客户端。不能把原有 `keys:read` token 悄悄解释为动态全权限。

此方案不应假设宿主支持额外 token exchange。保留宿主获得的同一 V2 bearer，由服务端每次验证客户端、批准挂载和目标助手；挂载完成即可在既有 MCP 会话调用。用实际 Codex/Claude/Grok 重新验证固定 scope 的接受、刷新、撤销与工具目录更新，不将标准允许视为宿主已兼容。

### 3. 助手与挂载

`assistants(assistant_id, account_id, legacy_space_id?, legacy_actor_id?, status, policy_revision, ...)`

`assistant_policy(assistant_id, messages_enabled, key_read, key_write, key_create, key_resource_selector, revision)`

`assistant_mounts(mount_id, assistant_id, installation_id, approved_by, approved_at, revoked_at, revision)`

挂载没有 scope/permission 列。family 是实例的一个活跃凭据，family 轮换不创建新助手。撤销一个 family 终止该凭据；解除挂载阻止这个实例继续代表该助手；禁用助手阻止所有挂载。

### 4. 多助手上下文不能依赖可变的全局“当前助手”

一个安装挂载多个助手时，每个业务调用显式携带 `assistant_id` 或服务端签发的上下文句柄；服务端先验证活跃 mount 再解析。名称不是授权依据。只有挂载恰好一个助手时才可省略。

禁止 `set_current_assistant` 改变整个 OAuth family 的全局身份：并发 thread 会相互切换并误用密钥。会话注册可存助手归属，后续请求携带工作会话 ID 时服务器可据此推导助手，但必须校验该会话与请求客户端关系。用户已允许同一助手 thread 共享权限；不需要每 thread token。

同一安装挂载多个不同权限助手意味着这份客户端凭据可以访问其已批准挂载的全部助手。客户端内上下文标识用于准确归属，不声称隔离同一客户端中的恶意 thread；若将来需要隔离不同助手，使用不同客户端配置/凭据存储，而不是伪称 thread 参数能完成安全隔离。

### 5. 权限动态生效与跨数据库一致性

每次消息和密钥操作查询当前助手政策、mount 状态、family 状态。缩减政策后，新的授权决策立即拒绝；已经解密交付的数据不能追回，已经获准并提交的外部操作无法取消。不要承诺撤销在网络中途能撤回秘密。

控制 DB 发布 policy_revision；keys DB 写入同 revision 的助手密钥政策。更新流程先准备 keys DB 新版本，再以 expected_revision CAS 发布控制 DB 版本。只有两端版本完全匹配才放行；失败与中断可重试，孤立准备记录不具权限。缩减或禁用可先在控制 DB 关闭访问再同步，避免等待同步仍暴露旧权。

每个密钥读操作在解密/返回前复查授权；写操作以权限版本/operation ID 进行最终检查。D1 跨 DB 无单一事务，严格线性化到跨 DB 提交瞬间的撤销无法靠两次 SELECT 保证；若业务要求此级别，需将政策写入与关键操作串行协调（例如单助手 Durable Object）或重新部署同库策略。首版应明确“撤销完成后开始的新请求被拒绝；已授权在途请求可能完成”的语义，并验证不出现旧 revision 重新生效。

助手权限修改页面显示受影响的所有已挂载客户端。增加权限需要用户在该页面确认一次，不逐客户端再设权限；缩减不等待客户端刷新。

## 迁移：不得静默扩大旧权限

1. 添加 V2 表与版本化鉴权路径，旧 OAuth family/token/audience 保持原语义；旧连接仍按旧 grant 检查，这是兼容层，不是 V2 新增第二套权限。
2. 每个旧 agent actor 建助手映射，保留 actor/message ID 和原 Space；默认 owner/user actor 不自动包装成用户助手。多个 consumer 的独立处理状态保留，只标明拟合并方案，不直接覆盖已读/完成状态。
3. 对同一个 Actor 的多个旧连接计算授权差异清单。禁止 grant 并集后把全部旧客户端自动挂载到扩大后的助手。默认保留旧路径，用户选择助手政策并明确批准迁入的客户端后再建立 V2 mounts。
4. 可无权限扩大证明的迁移候选也只能作为预填；实际新增持久挂载依照操作时确认要求。客户端标签同名、邮箱相同或同一机器名都不能作为合并证据。
5. 密钥 ciphertext、AAD、entry_id、issuer/subject 不重写；新助手政策引用原账户资源。新建权限单独保留，不能从 write 推导；全量范围应明确包含现有与未来条目。
6. 新旧协议同域名可并行，依据凭据版本分发；无法识别旧 scope 时拒绝而不是降级放行。旧服务和数据保留。最终清理需迁移完成、旧连接使用确认和单独退役任务。

## 最小验证清单

- 无助手的首次 OAuth → identity 可用；消息/密钥拒绝；创建草稿不授予权限。
- 恶意客户端指定未挂载 assistant_id、他人 session_id、其他账户助手均拒绝。
- 单安装两助手并发 thread 不串身份；多个 thread 同助手共享同一政策。
- 刷新不改变 mount/assistant；撤销 family、卸载 mount、禁用 assistant 分别验证。
- read/write/create 四种组合及资源范围；仅 create 不回显已有秘密，不默认扩大到 read。
- 政策缩减后持有旧 access token 仍被拒绝；政策增加需审计确认；keys DB 版本滞后拒绝。
- 旧不同 grant 迁移不取并集；旧密文 AAD 不变；回滚旧服务仍可处理原凭据。
- Codex、Claude、实际 Grok 产品各完成首次接入→挂载→工具调用→刷新→撤销；协议 fixture 只算隔离验证。

## 真正需要向用户对齐的问题（不重复已确认模型）

1. 助手密钥范围默认是否为账户全部现有与未来密钥？用户已确认读写新建，但“哪些密钥、未来条目”仍需页面明示；建议默认全量，保留高级范围。
2. 一个客户端已挂载多个助手时，是否接受它具备代表这些助手的全部能力？推荐接受且不做 thread 隔离，但要避免把上下文选择误称安全隔离。
3. 助手政策修改会同步影响所有已挂载客户端，包括旧机器；推荐修改页面统一确认影响，不逐客户端重复授权。

上述第 2/3 点可在主方案中作为明确默认说明，若用户既有确认已足够，不必重复问。首版资源范围、旧授权差异迁移和真实宿主协议可行性是主要待确认/待验证项。
