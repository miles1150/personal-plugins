# 消息路由与桥接服务改造研究

日期：2026-10-31（Asia/Singapore）。范围：只读源码研究和方案，无部署、无授权、无真实设备操作。核对本地 `5cd69447a61f4ec002b1aab5df9d79c0c7089889`；父任务已核对远端 main `95d5830253923bdac6afe8cb147ce1fc21f3dcc1`。本报告不把代码存在等同于线上版本或端到端验收成功。

## 已确认的产品约束

- 助手是唯一业务权限主体，挂载客户端；连接只承担凭据生命周期，不另设业务权限。
- 助手有多个工作会话；同一助手的会话不隔离权限。会话归属仍需校验，防止错误路由，不把它包装成安全隔离。
- 默认会话可空；没有默认时助手级消息积压。删除默认会话时替换为另一个合格会话，无候选则置空。暂时离线不等于删除，不自动切换。
- 发件人可只指定助手，也可指定该助手公开注册的工作会话。显式会话目标不能静默改投。
- 消息对话（跨助手往来）和工作会话（Codex thread/Claude session）必须分开。

## 当前实现证据

以下路径均相对仓库根，结论来自代码而非旧交接：

| 证据 | 当前行为 | 改造含义 |
|---|---|---|
| `services/waypost/migrations/0001_identity_mailbox.sql:7-25` | Actor 拥有多个 Consumer；Consumer 有 inbox/space_log 视图及过滤器 | 默认内部 Consumer 可以保留兼容，但不能以每个 thread 创建 Consumer 的方式建模工作会话 |
| 同文件 `threads`、`thread_members` | 现有 thread 是消息对话，成员是 Actor | 不得把它重命名/直接映射为 Codex thread |
| `migrations/0002_receipts.sql:2-23` | 回执唯一键是 Consumer+item；checkpoint 还按 view_revision、grant_fingerprint 分区 | 消息处理进度与分页游标不同。共享助手处理记录时，不能让某一客户端的扫描游标成为所有客户端的未读/执行起点 |
| `migrations/0020_receiver_claims.sql`、`src/service/receiver_claims.rs:25-121` | claim key 包含 Space/Actor/Consumer/item；条件 UPSERT 获得租约，generation 递增；attempt 去重 | 旧的多个 Consumer 可各自取得同一消息的 claim。目标模型需要助手级领取权或单一 canonical Consumer 并封堵旧写入口 |
| `src/service/receiver_claims.rs:124+` | complete 校验 attempt/generation/request_id/租约及当前授权；业务幂等 request_id 可稳定重用 | 保留 fencing；增加 assignment generation，不能仅检查线程名。外部操作不保证 exactly-once |
| `bridge/src/main.rs:245-301` | 私有 SQLite、进程锁、FULL sync；binding 钉住 origin、身份、adapter 和 thread；中断 sending 转 unknown | 不可通过换配置并删账本来迁移默认会话；否则重复通知，且旧 unknown 丢失 |
| `bridge/src/main.rs:344-411` | 主动 POST inbox/read 分页；将 item 和 cursor 同事务落盘 | 目前云端到 bridge 是拉，不是服务端主动 webhook |
| `bridge/src/main.rs:434-505` | 先 describe、receiver proof、消息可见性检查，再发送引用；结果 accepted/unknown/pending | accepted 只是 adapter 接受，不是读取或处理回执 |
| `bridge/src/main.rs:580-613`、`bridge/README.md` | 活跃轮询 30 秒，空闲 60 秒，加最多 5 秒抖动；失败退避 | UI 要显示轮询接收及延迟；不能称实时推送 |
| `bridge/src/receiver.rs`、`src/service/receiver_bindings.rs:1-185` | challenge 绑定 Space/Actor/Consumer/target；接收者以自身凭据确认；10 分钟挑战、24 小时证明；查询时验证接收凭据仍有效 | 有真实身份匹配机制，但不证明消息已读，也不持续证明宿主后续配置未变。新方案需绑定助手/实例/会话版本 |
| `bridge/src/oauth.rs:236-377` | 独立 PKCE 登录、凭据原子保存、刷新锁、refresh_pending 先落盘；刷新结果未知不重放旧 refresh | 已有自身 OAuth，不能按旧“只有 token_file”的交接重写；保留安全恢复语义 |
| `bridge/src/codex.rs:25-134,227+` | 固定 thread queue；通知只含引用；严格解析 queue receipt；unknown 可用官方 app-server 查 marker | 可以复用 adapter；thread 标记只能证明通知出现在会话，不证明任务已执行 |

## 推荐结构：每个本地运行环境一个 bridge supervisor

首版保留现有 Rust 二进制与适配器，增加安装级 supervisor，内部按助手/挂载保持独立任务槽和账本分区。不为每个 thread 装一个系统服务；也不让一个助手只能有一个 bridge 进程。

运行环境用持久随机 `installation_id` 标识（按 OS 用户/独立配置目录；不是硬件指纹）。每个 OAuth 客户端实例仍独立挂载到助手，凭据不能因为“同一机器”自动共用。supervisor 仅调度它已明确挂载的助手。

桥接服务是可信客户端组件，不是新的助手，不拥有额外业务权限。由于用户选择助手权限统一，桥接凭据若代表助手就继承其权限；实现仅调用消息接口不代表它的凭据是只读。应优先在同一受信本地运行环境内复用挂载凭据的 broker 接口，私密落盘和令牌轮转由单一组件管理。若必须独立 OAuth，则它是另一份同助手挂载凭据，可单独撤销；UI 如实展示，不暗中增加连接权限层。不能把宿主 token 拷给 bridge。此权衡应在技术方案披露，不凭空承诺无需单独登录。

云端继续 Worker + D1 + R2：D1 保存权威分配、租约、处理状态；R2 保持消息载荷；本地 SQLite 记录通知尝试。无证据表明个人项目需要立刻引入 DO 或 Queue。未来需要更短延迟可加持久连接提示，提示丢失仍由拉取补齐，权威状态仍在云端。本报告使用 Cloudflare skill 作产品选型，不涉及具体新 API/额度承诺。

## 新增的最小领域记录

1. `work_sessions`：stable session_id、assistant_id、client_instance_id、host_kind、host_session_ref（仅本地解析）、display_name、discoverable、state、capabilities、registration_revision、created_at。生命周期 active/deleted 与在线观测 online/offline/unknown 分字段。session_id 不复用。
2. `assistant_routes`：assistant_id、nullable default_session_id、revision。默认切换、删除与候选选择做条件更新，防止并发删除造成悬空默认。
3. `message_targets`：message_id、recipient_assistant_id、nullable requested_session_id、target_mode=assistant/session。保存发送者意图，不随默认修改；一个消息可有多个助手收件人，各自独立目标。
4. `delivery_assignments`：delivery_id（消息+收件助手）、assigned_session_id、assignment_generation、reason、state、created_at。与原始目标分开。消息对话可存 sticky session 路由，但不能覆盖新消息显式指定的目标。
5. `processing_claims`：delivery_id、attempt_id、claim_generation、assignment_generation、holder_session_id、lease_expires_at、completed_at、operation_request_id。唯一性以消息在助手中的一次投递为边界，不按客户端或 Consumer 分裂。
6. `wake_attempts`：delivery_id、assignment_generation、adapter_attempt_id、target_session_id、pending/sending/accepted/unknown/not_attempted、adapter_receipt。可先存在本地；必要的健康摘要上报云端，不把通知回执当业务回执。

会话公开显示只是可寻址和发现规则；同助手所有会话拥有相同权限，指定目标不意味着同助手其他会话无权读取。服务端对 claim 的 session/assignment 校验是工作协调约束，不伪称 thread 安全隔离。原生 MCP 无法可信识别当前 thread 时，客户端提交的会话标签只能标识路由；应以 bridge 注册映射和宿主证据核实可唤醒能力。

## 分配规则与边界

### 普通发送

- 显式指定会话：发送事务校验会话存在、属于目标助手、允许被当前发件人发现/寻址；若已删除返回 `SESSION_UNAVAILABLE`，不能成功后偷偷改投。暂时离线允许接受，显示等待。
- 仅指定助手：先使用该消息对话在该助手中的有效既有负责会话；无有效既有负责会话时使用默认；均无则 unassigned。这里“继续既有会话”仍是待确认产品规则，不应藏在实现里。
- 用户更改默认仅影响未来未分配的新工作；不能搬走正在处理的工作。此前无默认而积压的未分配消息可按明确的接管动作批量分配；是否设置默认即自动接管积压，需确认。
- 显式指定会话的消息若与既有对话负责会话冲突，首版不在同一执行链并行运行：标为待转交，或建立可审阅的分支工作。推荐显示“本次改由 X 接收”，不暗中改变整段对话的默认归属。

### 删除默认会话

删除为 tombstone，不删除历史消息或执行记录；标记会话不可新投递，与更新 assistant_routes 在同一原子事务发生。候选必须同助手、未删除、仍有效挂载且具备接收能力；离线候选不等于无效，但是否应优先在线者不能令短暂心跳抖动改变稳定选择。

建议固定可解释顺序：用户预设替补优先，其次最近成功接收的合格会话，再按创建时间稳定排序。不要求用户管理替补清单；有多个候选时删除确认页展示系统选择并可更改。没有候选 default=null。

未指定会话且尚未开始的旧分配可以重新分配，但必须 increment assignment_generation，并使旧 claim/完成请求失效。显式指定被删会话的未完成消息保持 `needs_reassignment`，请求操作者明确转交；转交不改写原始收件目标。已开始或 unknown 的工作必须先核查外部执行结果，不能因删除默认自动重跑。

### 主动拉取与无后台唤醒

Grok 网页/其他受限客户端可注册 pull-only 工作会话。其地址仍可公开，但列表显示“需主动查收”；不伪装成后台在线。没有真实 thread ID 的宿主注册“手动查收会话”，不伪造物理设备或可恢复上下文。每次查询显式声明助手/会话上下文，并通过挂载关系校验；工作领取依然走统一助手级 claim。

## bridge 执行链与恢复

1. 启动：恢复 installation_id、私有账本和 OAuth；检查挂载有效性；读取会话注册和 route revision。不通过名字猜旧身份。
2. 拉取“分配到此运行环境”的投递通知；先事务写入本地引用和分页游标。服务端 assignment 是事实来源；客户端游标不是处理完毕证明。
3. 发唤醒前：核对助手、挂载、session registration revision、assignment_generation 与 receiver proof。证明失效暂停该会话，不阻断其他会话。
4. 给宿主发送不含正文和密钥的引用，携带 delivery_id、目标助手/会话、assignment_generation。本地先记 sending；响应不确定转 unknown，不盲目重发。
5. 接收者查自身助手上下文，读取消息，使用 assignment+claim generation 领取。只有领取成功者执行；设置 received 回执，不把查列表当收到。
6. 长任务需要显式 renew/heartbeat（当前 claim API 没有 renew，租约最长 300 秒，不能覆盖常见长任务）。租约到期不自动意味着外部工作已停止；需要恢复核查或业务幂等。
7. 完成提交必须带完整 fencing 上下文，服务端事务检查分配版本、领取版本、挂载有效、助手当前权限；成功写 processed。迟到旧进程的 complete 应返回 claim_lost，而不是把新负责人工作标完成。
8. unknown 通知优先 adapter reconcile；查到 marker 只标通知 accepted。未查到不能证明未发。外部动作使用稳定 operation_request_id；无法去重的不可逆动作在结果不明时进入待核查，不自动重试。

产品状态必须分别显示：已入收件箱、等待分配、等待客户端、通知已接受、已领取/处理中、处理完成、需要核查。消息阅读回执与处理回执也分开。UI 不需要把每个内部状态都放主列表，但诊断详情应能回答停在哪一步。

## 兼容迁移顺序

1. 新增 schema 和会话表，不改已有 Actor/message/request_id/密文 AAD。旧 Actor 经明确映射成为助手；旧 Consumer 保留 ID 与历史。
2. 为每个助手建立 canonical 处理边界，扫描旧多个 Consumer 的回执/claim 差异：已完成优先保留，但不能将“通知 accepted”迁成 processed。冲突/unknown 输出待核查清单。
3. 升级所有会影响同一助手的 receipt/claim 写入口到助手级协调，或冻结该助手的旧自动 receiver 写入再切换；只迁新 endpoint 而让旧 endpoint 继续按 Consumer claim 会破坏去重。
4. 为旧固定 bridge 建立一个客户端实例、一个工作会话和显式旧 binding 映射。先备份账本，导入 items 状态与 adapter receipt；unknown 原样保留，cursor 可失效后从引用账本去重扫描。迁移有版本和校验计数，不能删 state_dir 重建。
5. 新 supervisor 与旧 bridge 不同时主动派送相同助手。先只读 shadow 路由对比，再停旧、切新。回滚保留新增表/历史，恢复旧进程前确认新系统派送记录不会被重复发送；不可简单启动旧二进制即宣称回滚安全。
6. 新默认/公开会话功能按助手开启，旧 sender 未指定会话仍能送到原 Actor。显式会话字段只在新服务识别，不能靠旧协议丢弃未知字段后误路由。

## 精简而必要的验收

单元/本地 SQLite：目标意图与分配分离；默认空；删除默认原子替换；显式目标删除不改投；双 worker claim；迟到 completion 被 fencing 拒绝；shared assistant 多 thread 不重复执行；旧账本迁移 unknown 不丢；权限撤回阻止新读写。

真实 D1 隔离环境：并发删除/分配/领取、幂等重试、租约更新与转交竞争。只覆盖会产生重复处理或丢消息的核心，不堆全面集成矩阵。

Mac mini：两个真实 Codex thread 注册为同助手会话→默认路由→显式路由→离线积压→删除默认替换→重启 bridge→验证 received/processed 与一次业务效果。另测 OAuth 刷新、挂载撤销、错误助手 receiver 阻止。消息和密钥使用合成数据，不记录凭据。

Claude/Grok：先判定能否可信获取会话、能否后台唤醒；有能力则测真实会话；无能力仅验收主动查收并如实标注。不得以模拟 adapter 通过声称真实宿主支持。

## 需要父任务汇总的产品问题

高优先级，只问决定产品行为的问题：

1. 无默认时积压的消息，在后来设置默认后是否自动派送？建议未开始的助手级消息自动接管，显式会话目标和结果未知的工作除外，并展示数量。
2. 发到已有消息对话但未指定会话，继续原负责会话，还是每条都走当前默认？建议继续原会话，避免丢上下文；用户明确改投优先。
3. 同一消息对话被显式指定到另一个会话，是否表示整段工作交接，还是只投递本条？建议只改变本条目标，若原工作正在执行则提示交接，不默默并行。需要统一到完整方案。
4. 默认会话删除后的替补规则是否接受“最近成功接收的合格会话，删除时可更改”？这里需避免随机选择，通常不必让用户预先配置优先级。
5. 工作会话“注册即公开”还是默认私有、主动发布后其他助手可选？用户说“也可以注册，并在其他助手收件人展示”，未明确发布默认；建议注册流程一个“在收件人中显示”开关，默认开启或关闭须统一产品决定。

技术团队自行处理、不应转嫁用户：fencing字段、D1事务策略、游标过期处理、private file权限、未知刷新不重放、默认选择稳定排序、指标脱敏、bridge进程分区。
