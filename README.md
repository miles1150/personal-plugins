# personal-plugins

个人维护的 agent 插件与配套工具集合。

## 私人插件市场

当前源码准备 `miles-works` 0.7.0，尚未发布；已有市场分发为 0.6.0。该插件为 Codex 和 Claude Code 提供一个远程 MCP（`https://works.miles1150.top/mcp`），统一消息和密钥工具，权限分别展示；仍包含独立 CLI 的本地 Keyenv 技能。市场只保留 Miles Works；旧独立 Waypost / Keys 托管服务已退役，旧地址和授权不可继续使用。安装来源与当前验收状态见 [插件市场指南](docs/plugin-marketplace.md)。安装不会自动登录、授权或启动后台接收，登录由用户本人完成。

Waypost 原生 MCP OAuth 与 bridge 的新机接入方案和隔离实验见 [接入准备](docs/waypost-native-onboarding.md)。

## Waypost 服务

Waypost 服务端、共享 MCP 网关和 bridge 位于 [services/waypost](services/waypost/README.md)。它们为统一 Miles Works 服务提供共享实现；当前部署见 [统一部署说明](services/waypost/docs/unified-deployment.md)。[迁移说明](docs/waypost-monorepo-migration.md)保留为历史记录，不是重新部署旧服务的指令。

## Keyenv

面向终端、脚本和 agent 的凭据工具：本机加密，通过 Google Drive API 同步，以环境变量供程序使用。支持隐藏输入 K=V，以及用户选择启用的登录 zsh 自动加载。

| 你的目标 | 从这里开始 |
| --- | --- |
| 其他用户安装 CLI，使用自己的 Google 客户端和密钥库 | [通用安装](docs/keyenv-installation.md) |
| 本仓库维护者在新电脑接入已有个人库 | [个人新电脑接入](docs/keyenv-local-handoff.md) |
| 录入、更新、同步、让程序或 agent 使用密钥 | [日常使用](docs/keyenv-usage.md) |
| 开发、测试、实验探针、发布 | [开发说明](docs/keyenv-development.md) |
| 查看已验证与待验证范围 | [验证状态](docs/keyenv-validation.md) |

CLI 的安装和使用方法可复用；个人接入包只服务于维护者自己的 OAuth 客户端与现有库，不是其他用户共用的登录服务。通用安装不需要个人接入包，也不需要知道维护者的主密码。

当前提供实验性 [0.1.0-rc.1 预编译包](https://github.com/miles1150/personal-plugins/releases/tag/keyenv-v0.1.0-rc.1)，支持 macOS/Linux 的 arm64/amd64，无需 Go、Xcode 或钥匙串。单机真实 OAuth、Drive、录入及恢复已有验证记录；双 Mac 与长期授权仍待验收，不能据此宣称稳定版。

主密码只由本人在终端隐藏输入，不保存主密码。仓库可分发代码、文档、公开客户端配置和经授权的客户端密文；完整 client.json、服务密钥、OAuth token、本机解锁与登录缓存不进入 Git。

更多入口见 [Keyenv README](tools/keyenv/README.md)。CLI 不依赖 agent 品牌；仓库中的 [skill](plugins/miles-works/skills/keyenv/SKILL.md) 是可移植操作说明，尚不代表原生插件已经注册。

## keyenv-remote

远程密钥功能现在通过 Miles Works 的 `https://works.miles1150.top/mcp` 使用，安装 `miles-works` 后由用户完成登录与授权。旧独立 Keys 服务及入口已退役。`keyenv-remote` CLI 的旧密钥专用 OAuth 流程尚未适配统一服务，不能只替换 URL 后继续使用；本机 `keyenv` 工具不受影响。当前支持范围见 [远程密钥说明](docs/keyenv-remote.md)。

## Codex CU MCP

将本机 OpenAI 的 Chrome Browser Use 与桌面 Computer Use 接入 Claude Code 等 MCP 客户端，统一入口为 `codex-cu`。支持独立会话元数据、应用授权请求转发与轮次清理；属于依赖已安装桌面运行时的非官方本地适配器。安装、能力范围与验证说明见 [Codex CU MCP README](tools/codex-cu-mcp/README.md)。
