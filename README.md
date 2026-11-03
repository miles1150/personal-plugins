# personal-plugins

个人维护的 agent 插件与配套工具集合。

## 私人插件市场

当前源码准备 `miles-works` 0.8.0，尚未发布。该插件为 Codex 和 Claude Code 提供一个远程 MCP（`https://works.miles1150.top/mcp`），统一消息、密钥和设计分享工具，权限分别展示。市场还以固定提交引用上游的 `baoyu-design`；旧独立 Waypost / Keys 托管服务已退役，旧地址和授权不可继续使用。安装来源与当前验收状态见 [插件市场指南](docs/plugin-marketplace.md)。安装不会自动登录、授权或启动后台接收，登录由用户本人完成。

Waypost 原生 MCP OAuth 与 bridge 的新机接入方案和隔离实验见 [接入准备](docs/waypost-native-onboarding.md)。

## Waypost 服务

Waypost 服务端、共享 MCP 网关和 bridge 位于 [services/waypost](services/waypost/README.md)。它们为统一 Miles Works 服务提供共享实现；当前部署见 [统一部署说明](services/waypost/docs/unified-deployment.md)。[迁移说明](docs/waypost-monorepo-migration.md)保留为历史记录，不是重新部署旧服务的指令。

## 远程密钥

密钥只保存在 Miles Works（`https://works.miles1150.top/mcp`），安装 `miles-works` 后由用户完成登录与授权，助手按获准范围读写。旧独立 Keys 服务及入口已退役。本机 `keyenv` 命令行（本机加密库与 Google Drive 同步）及其 `keyenv-remote` 客户端已从仓库移除；已发布的 `keyenv-v0.1.0-rc.1` 预编译包不再维护。当前支持范围与安全边界见 [远程密钥说明](docs/keyenv-remote.md)。

## Codex CU MCP

将本机 OpenAI 的 Chrome Browser Use 与桌面 Computer Use 接入 Claude Code 等 MCP 客户端，统一入口为 `codex-cu`。支持独立会话元数据、应用授权请求转发与轮次清理；属于依赖已安装桌面运行时的非官方本地适配器。安装、能力范围与验证说明见 [Codex CU MCP README](tools/codex-cu-mcp/README.md)。
