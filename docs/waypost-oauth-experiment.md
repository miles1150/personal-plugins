# Waypost OAuth 宿主兼容实验

这是为隔离原型准备的配置与验收方案，不会启用服务，也不代表现有 Waypost 生产服务支持 OAuth。OAuth 原型由后端另行实现；本环境未取得其分支源码或发现文档，因此这里只能对照任务提供的能力和官方宿主文档，无法确定原型的确切端点、发现字段或回调校验规则。现有独立凭据接入方式不受此文件影响。

两个模板是用户私有宿主配置的示意，不是插件清单，也不会自动加载：

- [Codex TOML 模板](../examples/waypost-oauth/codex.toml.example)：预注册 public client，固定 loopback 端口。其简单 `/callback` 路径只适用于授权服务器公布 issuer 响应支持、并在授权响应返回匹配 `iss` 的情况。
- [Claude Code JSON 模板](../examples/waypost-oauth/claude.json.example)：预注册 public client 和固定端口。当前官方文档给出的回调是 `http://localhost:31572/callback`；具体版本可能不同，应在该版本实际登录前核对。

模板中的 URL、client ID 和端口均为占位示例；不要原样连接或把 bearer token、refresh token、client secret 写进仓库。为每个宿主向**测试**授权服务器预注册明确的客户端及回调，优先选择 PKCE public client 和 `token_endpoint_auth_method=none`。本原型不支持 DCR，不应把 DCR 注册端点配置为兜底；如服务端也不支持 CIMD，宿主必须配置预注册的 client ID。两宿主可以使用不同客户端，客户端 ID 本身不是登录令牌。

## 精确回调约束

Codex 的 `callback_url` 与 `callback_port` 要一致。服务端若在授权服务器元数据中提供 `issuer` 且声明 `authorization_response_iss_parameter_supported=true`，配置的完整回调可被复用，但授权响应必须返回匹配的 `iss`。不支持该机制时，Codex 可能忽略没有服务器专属后缀的配置，改为 `/callback/<server-specific-id>`；该 ID 取决于 MCP URL，不能按示例猜测。获取实际 URL 后，在测试服务器预注册它，匹配 scheme、主机、路径及实际端口。无端口的 `127.0.0.1` 配置还可能在授权时插入随机端口；服务端必须实现兼容的 loopback 匹配，否则明确固定端口并核验实际授权请求。不能因为相同的端口就认为 `localhost` 与 `127.0.0.1` 相同。

Claude Code 使用 `oauth.clientId` 和 `oauth.callbackPort`；官方文档也支持 `claude mcp add --transport http --client-id ... --callback-port ...`。`localhost:PORT/callback` 与 Codex 的回调通常不同，应分别注册；旧版本可能产生不同主机名。Claude 的预注册客户端支持不等于已确认该原型兼容。避免给 OAuth 连接额外配置 `Authorization` 静态头；Claude 可能将其视为另一认证机制并停止回退 OAuth。

先核对受保护资源元数据、授权服务器元数据、MCP URL 和 resource audience；然后在隔离环境检查授权码、`state`、S256 PKCE、客户端 ID 和精确回调，测试 token 交换、到期刷新、撤销后拒绝以及各宿主安全存储。`resource` 参数、scope、issuer 和 redirect 的具体支持须根据后端实际代码和发现结果确认，不能因客户端发出参数就认为服务端接受。客户端应用与登录用户的身份映射、权限边界也需服务端验证。生产环境要求可信 HTTPS 与正常证书校验；loopback HTTP 仅用于受控本地测试。

## 当前验证

`PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_waypost_oauth_mock.py' -v` 在有 Codex CLI 的 POSIX 主机上启动本机虚拟 MCP/授权服务器，使用临时 Codex 配置与虚拟客户端/令牌，不连接真实 Waypost。Codex CLI 0.159.2 对没有 DCR 的模拟服务器，分别完成带 issuer 响应和无 issuer 响应的预注册客户端授权码交换；检查 `client_id`、S256 verifier、resource、token 交换中的 redirect，并确认无注册请求。无 issuer 时观察到服务器专属回调后缀；声明 issuer 支持却不返回 `iss` 的负例在 token 交换前被拒绝。模拟服务器按测试假设实现，不等同后端原型；测试没有覆盖真实用户身份、刷新、撤销、Claude Code 或生产。

下一步必须取得原型提交及配置，逐项核对元数据 `issuer`、`authorization_response_iss_parameter_supported`、返回的 `iss`、预注册客户端类型和精确回调规则、可变 loopback 端口、授权的 scopes、resource/audience 和 token endpoint 请求格式。随后在隔离实例上分别用真实 Codex 和 Claude Code 完成登录、读取、刷新和撤销负例；未完成前不能宣称端到端兼容。

参考：[Codex MCP OAuth 与回调](https://developers.openai.com/codex/mcp)、[Codex 配置参考](https://developers.openai.com/codex/config-reference)、[Claude Code MCP OAuth](https://code.claude.com/docs/en/mcp)、[MCP 授权规范](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)。
