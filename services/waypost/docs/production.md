# Cloudflare 生产入口

当前使用统一 Miles Works：

- 账户与工作台：<https://works.miles1150.top/account>
- MCP：`https://works.miles1150.top/mcp`
- Worker：`miles-works-production`

旧独立 Waypost / Keys 托管服务、旧域名和数据资源已退役，不再提供邀请登录或回滚目标。旧凭据不能迁用到新 audience。

发布、配置、验收与回滚遵循[统一部署说明](unified-deployment.md)，模板为 [wrangler.unified.production.example.toml](../wrangler.unified.production.example.toml)。不要用本地 `wrangler.toml` 创建新的独立生产服务。

历史部署与版本验收仍见 [validation.md](validation.md)；其中旧地址、资源和版本只记录当时事实，不代表当前可用服务。新的领域模型 v2 尚属设计，不能将其助手挂载/会话路由视为当前线上能力。
