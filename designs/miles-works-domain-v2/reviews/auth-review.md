# 独立审阅：授权、迁移与实施边界

2026-10-31；审阅对象为当时的 DOMAIN.md、IMPLEMENTATION.md。prototype/SPEC.md 尚不存在，未据此声称审过交互原型。只读源码核对，不是安全测试或生产验证。

## 结论

核心模型可以接受：助手唯一业务权限主体、挂载无独立 ACL、thread 不隔离；bootstrap 不放行业务；多助手请求上下文显式；bridge 全权限如实披露；跨 D1 撤销语义明确；旧 grant 不取并集；迁移及回滚没有假装可无条件切回旧二进制。

有两项应在进入实现前补齐，均不需要再引入新产品实体。

## 必须补齐

### A1：现有共享资料工具的权限归属遗漏（高）

DOMAIN 四个复选框只涵盖消息、密钥，但当前 `services/waypost/src/service/mcp.rs:25-58` 除消息之外还暴露 blob_prepare/commit/get、memory_publish/read；`memory_publish` 允许 snapshot/delta 及 delete_ids。IMPLEMENTATION 的新工具清单未映射这些能力。

如果旧 `waypost_*` 统一路由继续以“收发消息”放行，会把修改/删除共享资料暗含在一个用户看不出的权限中。如果直接移除，则是既有能力回归。

建议首版明确：消息附件 blob 操作仅限助手可访问消息附件的范围；独立共享资料/memory 工具暂保留旧兼容协议，不向 v2 助手默认开放，后续作为助手的单独能力再讨论。替代选择是用户确认将共享资料纳入消息能力并改复选框文案，但不能静默包含。补齐 old-tool→v2政策→兼容路径对照表。

### A2：挂载批准必须绑定不可替换的客户端请求（中）

DOMAIN 已写用户批准，IMPLEMENTATION 只列 mount_request 和 mount approved_by/at，尚缺请求存储约束。首次 bootstrap 同一凭据升级为可用挂载是合理候选，但必须确保网页批准的是最初那个客户端，而不是某个客户端在确认后替换 installation/assistant 的参数。

建议新增 mount_requests 或等价受约束状态：request_id、account_id、installation_id、发起 family_id、目标 assistant_id、拟采用 policy_revision、expires_at、consumed_at；目标/发起方批准前后不可变。批准使用账户登录+CSRF+一次性请求、校验账户归属及请求仍有效；政策从预览到批准变化则刷新摘要。bootstrap 不允许指定另一个 installation_id。申请链接不是秘密授权凭据，本身不能完成批准。写入 mount 和消费请求原子进行。

不需要用户为这些实现约束做产品选择。

## 实施细化建议（不阻止领域模型确认）

- DOMAIN 说会话换助手“关闭旧注册、创建新注册”，又规定外部会话按 `(installation_id,host_kind,external_session_id)` 去重；SQL 唯一性应只限制 active 注册，或显式带注册世代，否则墓碑会阻止新注册。旧地址永不复用。
-固定协议 scope 必须新版本语义，不能把旧 keys:read/waypost scope 当作可动态获取全助手能力。建议 P0 输出实际 discovery/scope/consent 文案及宿主结果；首次只接入的说明要披露后续挂载及助手权限变化的语义。
- bridge 若独立挂载，应在原型客户端清单标为“后台桥接”，明确继承助手全部业务权限。主稿已经写出这个边界，后续设计不能缩写为“只收消息”。
- 每请求重新检查不能只覆盖 MCP 入口：附件下载 URL、blob 提交、REST旧路径、最后数据库 mutation guard 都要列出。临时已签发下载地址是否支持即时撤销需明确，不能对所有下载笼统声称下一次检查即拒绝。
- 同安装挂载多个助手、客户端持有全部这些助手权限已正确说明；thread 上下文选择只防误路由，不声称对抗同一客户端恶意 thread。
- 实施阶段“纯SQLite→真实D1→真实宿主”顺序合理；P0失败时应调整UX，而非把额外OAuth跳转藏进桥接脚本。

## 需要问用户的内容

1. 新 v2 是否暂不开放独立共享资料/memory 功能，仅保留消息附件和旧协议兼容？推荐先这么做，避免把收发消息扩大成共享资料管理。
2. 其余真正决策沿用总 QUESTIONS：密钥资源范围、会话公开默认、默认会话替换范围等。本审阅不重复询问已确定的助手唯一权限与thread不隔离。

## 验收补充

加入两条拒绝用例：批准页面打开后替换安装/助手/政策版本不得静默通过；仅消息助手调用 memory_publish/delete_ids 不应因旧 waypost 名称映射而获得权限。

## 复核追加（2026-10-31，保留首次审阅）

重新读取 DOMAIN.md、IMPLEMENTATION.md 和 QUESTIONS.md 后：

- **A1 已在方案层解决。** DOMAIN §4.2 与 IMPLEMENTATION §3 明确旧 memory/read/publish（含删除）不默认归入收发消息；消息附件限制范围；QUESTIONS 将共享资料是否本次统一列为附加范围选择。尚待用户范围决定，不是已经删去功能。
- **A2 已在方案层解决。** DOMAIN §4.1 明确不可变申请、账户 CSRF、一次性消费、政策变化重示摘要；IMPLEMENTATION 新增 mount_requests 约束。
- 有效注册唯一性、墓碑/新世代已明确；附件 URL 撤销不再过度承诺。
- 新旧协议由 client_families.protocol_version 和独立兼容路径区分，旧 grant 不自动扩大。P0 仍必须写出固定 scope 的明确新语义和真实宿主证据，不能将此项标成已验证。

授权/迁移方面无剩余领域设计阻塞。研发细节补充：消息附件一般先上传再发送，因此“可访问消息归属”应允许受助手拥有的待发送附件 reservation，再在发送时提交关联；不能要求上传开始时已经存在 message_id。过期孤立 reservation 可清理，但不得形成无限制独立资料库。

本次仍仅文档复核，没有执行协议、安全、生产或真实宿主验收。
