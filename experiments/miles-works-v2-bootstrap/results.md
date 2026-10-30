# v2 bootstrap 协议隔离实验

运行日期：2026-10-30（环境 UTC）。仅 loopback 合成数据；未访问生产、Google、真实用户凭据或模型 API。继承 `../miles-works-unified-mcp-2026-10-29` 的 OAuth/宿主实验，新增固定 `works:client` scope、按请求指定助手和在线挂载/策略检查。OAuth consent 和挂载批准均是实验驱动直接模拟的所有者动作，不是账户页面验收。进程内 token，临时宿主配置在结束时删除；fixture 不可用于生产。

- Codex CLI 0.159.2：实际 CLI 完成 DCR、PKCE、回调、兑换；app-server 工具调用验证无挂载时业务拒绝、申请后模拟批准两个助手、同一凭据分别调用，收紧 alpha 策略后 alpha 拒绝而 beta 可用，撤销 beta 挂载后拒绝。过期 access token 后宿主自动刷新；撤销 family 后拒绝。没有模型回合。详见 `codex-results.json`。
- Claude Code 2.1.312：实际 CLI 完成 OAuth、发现固定工具目录，过期后健康检查触发刷新，family 撤销后健康检查断开。未执行 Claude 模型回合或工具调用，不能声称批准后 Claude 业务调用通过。详见 `claude-results.json`。
- 直接 HTTP 边界实验：第二 family 即使知道助手 ID 也无法使用第一 family 的挂载；同 token 并发 20 次请求的 alpha/beta 显式上下文未串扰，策略收紧即时生效。fixture 为串行锁保护内存状态，不证明生产数据库并发语义。

工具目录在 bootstrap 时稳定暴露 identity、mount_request 和业务工具的 schema；真正可执行的只有 identity 和 request，业务工具返回 `ASSISTANT_REQUIRED`。因此该实验不证明隐藏业务工具目录的宿主动态重发现能力，也不泄露业务数据。实验中审批不是 HTTP 端点，不能证明网页登录、CSRF、申请绑定/过期/原子消费或 policy revision；这些必须在真实后端验证。未测 Grok，也未验证真实 Google 或真实部署。P0 的 Claude 模型工具调用与真实后端/账户页面仍未完成，不能将本实验标记为完整 P0 验收。

运行：`python3 verify.py`、`python3 host_verify.py codex`、`python3 host_verify.py claude`。Claude 使用相邻实验已安装的固定版本依赖（`node_modules` 链接）；临时目录为私有且独立。结果 JSON 不含令牌、授权码或内容值。实际后端集成需另行准备隔离服务、合成账户与安全的审批驱动，不可将本 fixture 的模拟通过当作后端验收。
