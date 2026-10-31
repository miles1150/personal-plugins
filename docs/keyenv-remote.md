# 远程密钥：Miles Works

当前远程密钥入口只有 `https://works.miles1150.top/mcp`，由 `miles-works` 插件提供。旧独立 Keys 服务及其域名、数据资源已退役；旧凭据不能直接用于新服务。

## 在宿主中使用

1. 按[插件市场指南](plugin-marketplace.md)安装 `miles-works`，或添加上述远程 MCP URL。
2. 用户亲自完成 Google 登录及 OAuth 授权；按页面确认消息、密钥读取、修改和新建的实际范围。
3. 回到宿主，先查询身份和能力，再用合成条目验证获准操作。安装成功、已登录、工具可调用和后台接收是不同验收项。
4. 访问 <https://works.miles1150.top/account> 管理现有授权。撤销不能追回已经读取的秘密或已执行的操作。

cloud session 是否复用宿主授权、Grok 不同产品的 OAuth/刷新能力，需要对应宿主实测，不能把统一 URL 当作兼容证明。操作说明见 [Miles Works 密钥 skill](../plugins/miles-works/skills/keyenv-remote/SKILL.md)。

## 本机工具的边界

本机 `keyenv` 继续管理本机加密库及 Google Drive 同步，见[日常使用](keyenv-usage.md)，不受旧远程服务退役影响。

`tools/keyenv/cmd/keyenv-remote` 是旧远程服务的 CLI。它的默认 origin 和仅申请 `keys:write` 的 OAuth 流程面向已退役独立服务；当前统一服务仍要求消息身份，不能仅把 `--server` 换成新 URL 就声称 `login/set/import` 可用。该 CLI 的统一接入尚待适配与真实验证，本指南不推荐用它登录或导入生产密钥，也不自动迁移其本地凭据。

## 当前安全边界

- Cloudflare Worker 持有数据密钥，D1 保存密文；这是服务端加密，不是零知识存储。
- 用户批准的宿主可以在授权范围内读取值。工具取值会进入宿主与模型上下文，不能靠 skill 保证秘密不可见。
- 写入工具仅应返回条目标识及字段名，不回显秘密；日志、报告和仓库不得包含值、token 或 OAuth code。
- 密钥数据库独立于消息数据库，但同一 Worker 可访问两者，不是进程级隔离。
- 当前生产还是现有连接授权模型；助手唯一权限主体的领域模型 v2 尚未实施。

部署及当前鉴权边界见[统一部署说明](../services/waypost/docs/unified-deployment.md)。[早期安全评审](research/agent-secrets-review.md)保留旧独立产品研究背景，其中地址与部署模板不再是操作入口。
