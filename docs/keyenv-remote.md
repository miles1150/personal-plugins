# 远程密钥：Miles Works

当前远程密钥入口只有 `https://works.miles1150.top/mcp`，由 `miles-works` 插件提供。旧独立 Keys 服务及其域名、数据资源已退役；旧凭据不能直接用于新服务。

## 在宿主中使用

1. 按[插件市场指南](plugin-marketplace.md)安装 `miles-works`，或添加上述远程 MCP URL。
2. 用户亲自完成 Google 登录及 OAuth 授权；按页面确认消息、密钥读取、修改和新建的实际范围。
3. 回到宿主，先查询身份和能力，再用合成条目验证获准操作。安装成功、已登录、工具可调用和后台接收是不同验收项。
4. 访问 <https://works.miles1150.top/account> 管理现有授权。撤销不能追回已经读取的秘密或已执行的操作。

cloud session 是否复用宿主授权、Grok 不同产品的 OAuth/刷新能力，需要对应宿主实测，不能把统一 URL 当作兼容证明。操作说明见 [Miles Works 密钥 skill](../plugins/miles-works/skills/keyenv-remote/SKILL.md)。

## 本机工具已移除

本机 `keyenv` 命令行（本机加密库与 Google Drive 同步）和连接已退役服务的 `keyenv-remote` 命令行已从仓库移除。目前没有让用户自己录入、不经过 agent 的入口：值通过宿主写入时会进入宿主与模型上下文。

## 当前安全边界

- Cloudflare Worker 持有数据密钥，D1 保存密文；这是服务端加密，不是零知识存储。
- 用户批准的宿主可以在授权范围内读取值。工具取值会进入宿主与模型上下文，不能靠 skill 保证秘密不可见。
- 写入工具仅应返回条目标识及字段名，不回显秘密；日志、报告和仓库不得包含值、token 或 OAuth code。
- 密钥数据库独立于消息数据库，但同一 Worker 可访问两者，不是进程级隔离。
- 当前生产还是现有连接授权模型；助手唯一权限主体的领域模型 v2 尚未实施。

部署及当前鉴权边界见[统一部署说明](../services/waypost/docs/unified-deployment.md)。[早期安全评审](research/agent-secrets-review.md)保留旧独立产品研究背景，其中地址与部署模板不再是操作入口。
