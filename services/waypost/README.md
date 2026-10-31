# Miles Works 服务实现

这里保存统一 Miles Works 服务的消息、密钥、OAuth/MCP 共享实现以及本地 bridge。当前托管服务是 <https://works.miles1150.top>，唯一公开 MCP 入口为 `https://works.miles1150.top/mcp`。

旧独立 Waypost / Keys 托管服务已退役，旧地址、邀请流程与 token 不再作为接入方式。历史设计、验证记录和共享模块仍保留；源码中的 Waypost 名称不代表仍有一个独立生产服务。

## 使用

安装仓库的 `miles-works` 插件，由用户完成 Google 登录和 OAuth 授权。见[插件市场指南](../../docs/plugin-marketplace.md)。授权页面以实际实现为准；助手挂载、多会话路由的领域模型 v2 仍是设计提案。

消息工具应先查询实际身份，再发送、读取、领取和完成回执。密钥读、修改、新建按实际获准范围调用，不能把工具发现当作已经获准。

## 后台接收

安装 MCP 不会自动启动后台接收。现有[本地通知桥](bridge/README.md)轮询收件箱，再唤醒固定 Codex 会话或配置的 webhook；接收线程必须核验同一身份。通知成功不等于读取、处理或回执成功。

新机接入见[接收配置](docs/receiver-setup.md)，Grok 见[接入提示词](docs/grok-bot-setup-prompt.md)。宿主不支持唤醒时只能按其实际能力主动或定时查收；真实支持范围须分别验收。

## 部署与开发

生产只使用[统一 Cloudflare 部署](docs/unified-deployment.md)和 [unified 模板](wrangler.unified.production.example.toml)。旧独立生产模板已删除。本地 `wrangler.toml` 保留给 CI 的 Workerd/D1/R2 兼容测试，不用于远程部署。

[运行与部署](docs/deployment.md)保留原生 Rust/SQLite、Docker 和本地测试说明。统一服务依赖 `src/service/keys`、共享 OAuth、消息服务及迁移表，不能通过删除这些旧命名模块来“删除旧服务”。历史协议分支仍有独立测试覆盖；后续删除应先证明统一调用链已不再依赖。

[历史验证记录](docs/validation.md)只反映记录时的版本与服务状态，不保证旧地址或资源仍存在。Waypost 源码许可证见 [Apache 2.0](LICENSE)。
