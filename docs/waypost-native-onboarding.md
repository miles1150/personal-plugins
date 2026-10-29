# Waypost 新机接入

源码中的 Waypost 0.4.0 插件为 Codex 和 Claude Code 提供远程 MCP 配置：宿主从服务发现 OAuth 元数据，自动注册客户端，并由用户在浏览器登录 Waypost、选择 Space / Actor / Consumer 和明确授权。无需手工输入 client ID。Google 凭据只由服务端持有；不要将 token、验证码或回调粘贴进聊天、日志或插件。客户端的显示名称不是信任证明，授权前应核对实际回调和所选身份。

当前市场仍固定已发布的 0.3.0，那个包没有上述连接。0.4.0 必须重新构建包含真实平台 bridge 的分发包，经各平台和宿主验收后更新市场 pin；当前分支不改变已发布包。已有旧版设备配对与新 OAuth 是独立连接，升级前检查重复连接，迁移和撤销须另行验收。

官方说明：[Codex MCP](https://developers.openai.com/codex/mcp)、[Claude MCP](https://code.claude.com/docs/en/mcp)、[Waypost 新机 OAuth](https://github.com/miles1150/waypost/blob/main/docs/mcp-oauth-setup.md)。Waypost 上游记录了隔离 Workerd 环境中 Codex 0.162.1 和 Claude Code 2.1.296 的 DCR、PKCE 与模拟账号授权；该记录不等于生产 Google 登录验收。

本分支 `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -v` 在 Linux 上通过 20 项：Codex 0.159.2 和 Claude Code 2.1.296 在临时市场安装插件并发现其 MCP 组件；两个真实 CLI 使用本地模拟服务器完成 DCR、PKCE 登录，其中 Claude 也通过安装后的 `plugin:miles-works:waypost` 完成同一流程。模拟服务不是 Waypost 后端，未验证工具调用、消息收发、Codex 安装插件登录或生产 Google 登录。测试时插件 endpoint 替换为本地 loopback，临时目录销毁，不连接生产。

## Bridge 的独立身份

插件中的 MCP 登录不会配置或启动 bridge；bridge 需独立身份、私有 token 文件、目标线程或 webhook 及持久状态目录。不得复制宿主的 access/refresh token 给后台进程。Waypost 的本地后端实现提供标准设备授权和旋转刷新，OAuth access token 可用于 bridge 所需 `/v0` 接口；但 bridge 本身尚无自动设备授权或刷新实现。后台凭据拥有空间成员的读写能力，并非只读。

安全实现需要用户确认设备授权、独立注册的公共客户端及其独立 token family；并使用跨进程锁和安全凭据存储。在刷新请求前持久记录进行中，成功后安全保存新 refresh token 和 access token，并处理跨存储崩溃一致性。刷新响应丢失或重启发现未完成请求时，禁止重放旧 refresh token，要求重新授权：服务端可能已消费旧令牌，重放会撤销整个 family。还须验证撤销、过期、状态目录恢复、日志不泄露凭据和 macOS/Linux 实际权限。

[离线状态模型](../tools/waypost/bridge_refresh_model.py)测试拒绝重复开始和不确定结果后的重放；它没有网络、锁、凭据存储或真正的设备授权，不是可运行的后台凭据管理器。目前生产 bridge 身份、设备授权和持续续期均未完成验收。
