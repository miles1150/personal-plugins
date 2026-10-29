# keyenv-remote：给 cloud session 用的远程密钥库

cloud session 里的 agent 通过远程 HTTP MCP（`https://keys.miles1150.top/mcp`）列出、取用、新增和修改密钥。会话里不装 CLI，也不填 client ID。人在自己的电脑上用 `keyenv-remote` 录入和从本机 keyenv 导入。现有 `keyenv` 继续管理本机文件库，两者互不替代。

服务端在 waypost 仓库（网关、OAuth、密钥产品），设计、部署和验证见 waypost `docs/design/0017-gateway-cimd-dcr-keys.md`。方案评审见 [agent-secrets-review](research/agent-secrets-review.md)。

## 组成

| 部分 | 位置 | 作用 |
|---|---|---|
| 密钥服务 | waypost，`WAYPOST_PRODUCT=keys` 的 Worker + D1 | MCP 工具、Google 登录、OAuth（CIMD/DCR）、按条目授权、AES-256-GCM |
| 插件 | `plugins/keyenv-remote` | Cursor、Claude、Codex 各一份清单，共用 `.mcp.json`（只有 MCP 地址）和一份 skill；不含二进制 |
| CLI | `tools/keyenv/cmd/keyenv-remote` | `login`、`set`（隐藏输入）、`import`；只装在自己的电脑上 |

## 在宿主里连接

1. 安装插件（`keyenv-remote@personal-plugins`），或在宿主里添加远程 MCP 服务器 `https://keys.miles1150.top/mcp`。
2. 点连接。浏览器打开 keys.miles1150.top，用 Google 登录。
3. 在同意页勾选这个连接能读哪些条目、能改哪些条目、能否新建。读和改分开：只需要取用就只给读；只给 `keys:read` 的连接即使勾了「改」也写不了。
4. 回到宿主。cloud session 能否复用宿主保存的授权取决于具体宿主；Claude Code、Cursor 和 Codex 的云会话尚需分别做真实端到端验收，不应预设可以复用。

客户端身份由宿主自己出示：支持 Client ID Metadata Document 的宿主（Codex、Claude Code）用它托管的文档，其他宿主走动态注册。插件清单和这份说明里都没有 client ID，也不需要用户手填。

在 `https://keys.miles1150.top/account` 可以看到所有连接并逐个撤销。撤销后下一次请求即失败。

## 本机 CLI

在本仓库构建（Go 1.26，仅本机）：

```bash
cd tools/keyenv && go build -o "$HOME/.local/bin/keyenv-remote" ./cmd/keyenv-remote
```

```bash
keyenv-remote login
```

浏览器打开同意页；服务器支持 CIMD 时 CLI 使用服务端托管的文档 `https://keys.miles1150.top/oauth/clients/keyenv-remote.json` 作为 client ID，否则在服务器提供 DCR 时动态注册公开客户端。回调是 `http://127.0.0.1:<随机端口>/callback`。CLI 默认只申请 `keys:write`；导入或覆盖已有条目时，在同意页勾选「新建」和「全部条目可改」。令牌保存在 `~/.config/keyenv-remote/credentials.json`（Linux）或系统对应的配置目录，目录 0700、文件 0600；`KEYENV_REMOTE_HOME` 可改位置。刷新令牌每次刷新都会轮换，CLI 用文件锁串行刷新。

```bash
keyenv-remote set github/token --purpose "CI pushes"
```

按提示逐行输入 `FIELD=value`（不回显），空行结束。条目不存在就新建，存在且本次登录可改就设置这些字段。输出只有条目名和字段名。

```bash
keyenv-remote import
```

从已解锁的本机 keyenv 导入一次。它调用 `keyenv list` 取元数据，再对每个条目执行 `keyenv run --credential NAME -- keyenv-remote __import-entry NAME`：值只通过子进程环境变量交给上传进程，不落盘，也不需要逐条确认导出。远端已存在的条目默认跳过，`--update-existing` 才覆盖；禁用或冲突的本机条目跳过。用途和过期日期一并带上；keyenv 的 `secret` 标记不保留，远端所有字段都加密。导入可能部分成功：逐条检查结果；任何条目失败或指定名称不存在都会返回非零退出码，重试时默认跳过已有条目。

`keyenv-remote logout` 只删除本机令牌；要在服务端结束授权，去账户页撤销。

## agent 的用法

见插件里的 [skill](../plugins/keyenv-remote/skills/keyenv-remote/SKILL.md)。要点：先 `list_entries`，名称不确定就问；`fetch_entry` 的结果写进仓库外 0700 目录里的 0600 JSON 文件（必须在仓库内时先确认 `git check-ignore` 命中），后续命令读文件，回复里只写路径和字段名；写入工具的结果只有名称；用户在对话里给的值录入后不复述。

## 威胁模型

**保护的资产。** 条目的字段值；其次是条目名、用途和字段名（暴露用户有哪些账号）。

**信任的一方。**

- Cloudflare：Worker Secret 里的数据密钥和 D1 里的密文都在它手里，所以它（或拿到账号的人）能解密。这是选择免费 Worker + D1 的代价，不是零知识库。
- Google：账号登录。拿到这个 Google 账号就能批准新连接。
- 用户批准过的宿主（Cursor、Claude、Codex 的后端）：保存刷新令牌，代 cloud session 调用工具。宿主被攻破等同于拿到这条连接的权限，范围以同意页勾选为限。

**控制。**

- 读写分离：scope `keys:read`/`keys:write` 与同意页按条目勾选取交集；未授权的条目不出现在列表里，取用时与不存在无法区分。
- 写入工具只返回名称；错误信息只含条目名和字段名；审计只记动作、条目名和客户端。
- 每条字段单独 AES-256-GCM，AAD 绑定账号、条目、字段和 kid；多密钥轮换。
- OAuth：只有公开客户端加 S256 PKCE；令牌按资源 `https://keys.miles1150.top/mcp` 签发，与 Waypost 的令牌不能互用；访问令牌 5 分钟，刷新令牌一次性轮换、重放即撤销整个族、族最长 30 天；授权响应带 `iss`。
- CIMD 拉取：只收带路径的 HTTPS 域名 URL，不跟随跳转，5 KB、5 秒上限，拒绝 IP 字面量和内网后缀；原生运行时另外拒绝解析到非公网地址的域名。DCR 只发公开客户端，总量和速率受限。
- 同意页显示客户端主机、回调主机，对只有本机回调的客户端和自报名称的 DCR 客户端给出警告。
- 只允许一个账号注册（`WAYPOST_ACCOUNT_SIGNUP_LIMIT=1`）；Waypost 的管理、Space 和设备码路由在密钥部署上不存在。

**不能防的事（纪律，不是沙箱）。**

- `fetch_entry` 的值会进入工具结果，也就进入模型上下文和宿主的会话记录。skill 只能要求 agent 不复述、不写进版本库和日志。
- agent 写临时文件时，值会出现在那一次文件写入调用里。
- 用户在对话里给的新值，在调用 `create_entry`/`update_entry` 之前就已经在上下文里。高价值的值请在自己电脑上用 `keyenv-remote set`。
- 被批准的连接在授权范围内可以读走值；撤销只能阻止以后的读取，已经读走的值要在上游轮换。
- 本机 CLI 的令牌文件靠文件权限保护；同一用户下的其他进程能读到。`import` 期间值在子进程环境变量里，同一用户可通过 `/proc` 看到。
- 本机回环回调可被同机进程冒用（MCP 规范的 localhost 冒用风险）；同意页只能提示，不能证明发起者。

**数据丢失。** 丢失 `KEYS_DATA_KEYS` 等于所有密文不可恢复；要离线备份这把密钥。D1 免费库的时间回溯能力和保留期以 Cloudflare 当前文档为准（本次未能核对）。
