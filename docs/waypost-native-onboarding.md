# Miles Works 新机接入

当前已发布分发为 `miles-works` 0.6.0；源码正在准备 0.7.0，尚未发布或完成真实宿主端到端验收。服务地址与安装来源见[插件市场指南](plugin-marketplace.md)。旧独立服务已退役。下面的 0.4.0 与模拟宿主测试段落是历史研究记录，不是当前安装步骤。

0.7.0 配套 v2 后端：用户亲自登录，在账户界面批准助手策略和客户端挂载；`works_identity` 显示可用助手，多助手调用需指定助手。工作会话由宿主注册，消息和密钥操作以助手策略授权。不能把安装插件、创建草稿或发出挂载请求当作批准。

源码中的 Waypost 0.4.0 插件为 Codex 和 Claude Code 提供远程 MCP 配置：宿主从服务发现 OAuth 元数据，自动注册客户端，并由用户在浏览器登录 Waypost、选择 Space / Actor / Consumer 和明确授权。无需手工输入 client ID。Google 凭据只由服务端持有；不要将 token、验证码或回调粘贴进聊天、日志或插件。客户端的显示名称不是信任证明，授权前应核对实际回调和所选身份。

当时市场的 `miles-works` 条目固定到包含真实平台 bridge 的 0.4.0 分发包；原有独立 `waypost` 条目仍是 0.3.0，那个包没有上述连接。已有旧版设备配对与新 OAuth 是独立连接，升级前检查重复连接，迁移和撤销须另行验收。

官方说明：[Codex MCP](https://developers.openai.com/codex/mcp)、[Claude MCP](https://code.claude.com/docs/en/mcp)、[Waypost 新机 OAuth](https://github.com/miles1150/waypost/blob/main/docs/mcp-oauth-setup.md)。Waypost 上游记录了隔离 Workerd 环境中 Codex 0.162.1 和 Claude Code 2.1.296 的 DCR、PKCE 与模拟账号授权；该记录不等于生产 Google 登录验收。

历史分支 `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -v` 在 Linux 上通过 20 项：Codex 0.159.2 和 Claude Code 2.1.296 在临时市场安装插件并发现其 MCP 组件；两个真实 CLI 使用本地模拟服务器完成 DCR、PKCE 登录，其中 Claude 也通过安装后的 `plugin:miles-works:waypost` 完成同一流程。模拟服务不是 Waypost 后端，未验证工具调用、消息收发、Codex 安装插件登录或生产 Google 登录。测试时插件 endpoint 替换为本地 loopback，临时目录销毁，不连接生产。

## Bridge 的独立身份

插件中的 MCP 登录不会配置或启动 bridge；bridge 使用独立 OAuth 登录、私有凭据和持久账本，不复制宿主 token。现有实现已有浏览器 PKCE、刷新锁、刷新未知结果恢复和接收身份核验，具体命令及限制以[bridge README](../services/waypost/bridge/README.md)为准。不要根据本文件旧隔离实验推断所有真实宿主已通过验收。

旧 token 文件不能只修改 base_url 后转用 v2 服务；须使用新 audience 的获准凭据，并核对助手、实例、工作会话及接收线程。后台凭据的能力以实际授权为准，不能把“只调用收件接口”误称只读权限。

保留账本并核查 unknown 状态；通知成功不等于消息读取或业务完成。用户完成登录，实机验证另记录身份、消息 ID、读取、领取和回执，不记录秘密。
