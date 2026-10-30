# 新线程交接 prompt：单 MCP 本地实现切片

在 personal-plugins 实际最新 main 基准上工作，先核验 HEAD 与远端 main；历史报告只作线索。阅读 `docs/miles-works-technical-feasibility-2026-10-29.md` 和本目录实验，区分已验证、建议、未验证。使用 sol high（由任务创建器实际设置；prompt 本身不能改变模型）。

目标：实现可评审的本地切片，让 miles-works 的一个规范 MCP resource 在同一 OAuth connection 下提供消息与模拟密钥工具，保留独立授权、即时撤销和既有幂等/回执语义。先沿用现有 transport 和业务实现，避免另造生产 OAuth 服务。实验 fixture 只能作验收参考，禁止直接部署为服务。

重点：当前 keys.is_some 全局切换路由/scope；Waypost Principal 与 KeysPrincipal 不同；两种 oauth family/store 不同；两子域 __Host 账户 cookie 不共享。设计 connection 父对象和 message/key grant 子对象，给出兼容 migration；普通连接不获得 owner/account 管理权。默认个人 Space 普通 agent、keys 为空；组合 consent 明确勾选，新增 scope 需要显式授权，refresh 不扩权。单独撤销 keys 后消息继续，撤销 connection 后两业务都停。既有条目授权与 all_read/all_write 的未来条目语义分别测试。

保留独立 keys 数据密钥/存储边界；禁止把外部 MCP token 原样转发给不同 audience 的后台，也禁止长期 owner-token 聚合。MCP 创建 Space/连接先形成 pending owner 审批，不返回管理 bearer。bridge 暂不安装，先完成凭证接口和刷新单写者设计，保留 state_dir/item_id journal；不把 queue accepted 记成 processed。

交付：工作分支 diff、schema/授权接口说明、真实代码集成测试、可重复命令与实际输出；在隔离 profile 用 Codex 实际调用两类工具并测刷新/撤销，Claude 至少发现工具与刷新/撤销。保留尚未验证的 cloud/桌面/Grok 项目，不冒充通过。若 Rust/Go toolchain 缺失，记录准确错误并尽可能在授权的本地环境补足。

不访问真实密钥，不读取生产凭证，不改生产、不合并、不部署。所有值用模拟数据；遇到未知结果先查证，不自动重试可能已执行的动作。完成可评审产物后汇报，不停留在概念方案。
