# Keyenv 第一轮验证记录

日期：2026-10-11（历史首轮）；最新状态：2026-10-12。

## 提交交接前最新复核（2026-10-12）

Air / Go 1.27.1 / darwin-arm64 实际复跑通过 `CGO_ENABLED=0 go test -count=1 ./...`、`CGO_ENABLED=0 go vet ./...`、正式 CLI 与 lab 构建、simulate 六项 true。均为本机合成测试，无真实 OAuth 或 Drive 授权。没有安装 Xcode，未复跑可选 race。另已通过打包/安装脚本语法、本机打包、临时目录安装与拒绝覆盖、CLI 命令清单、skill frontmatter/安全指引检查及 `git diff --check`；本次候选 29 文件的凭据模式扫描无命中（扫描并非完整泄漏证明）。构建产物和临时安装未进入 Git。

Google Cloud 条款、API 用户数据政策及 2SV 已完成；keyenv-personal 的 Drive API 已启用，Keyenv Local Desktop 已创建。用户知情后明确保留现有客户端。客户端 JSON 上次仍不存在，Downloads 旧 a.json 未核实；手工下载、真实 drive.file 授权、实际 Drive 与双机/正式用户 vault 验收仍待完成。没有计费或免费试用。新电脑接续以 [最新交接](keyenv-local-handoff.md) 为准，下方旧检查点描述保留为历史。

## 正式 CLI 首轮实现检查点（2026-10-12）

用户最新明确授权按现有设计实现正式功能，已覆盖此前仅限探针的范围；下文旧交接阶段限制保留为历史记录。没有运行用户真实初始化，没有输入用户正式密码、API key 或 OAuth token。

本次新增 `cmd/keyenv`、`internal/app` 和 `internal/terminal`：空本地初始化、复用 wrapped-key header 的终端解锁、加密绑定/密文状态保存、macOS/Linux 进程锁、Drive setup/明确 folder 接入、add/update/disable/list/inspect/resolve、run 环境注入、交互确认导出、sync/status/doctor、加密备份及新目录恢复。正式 revision 使用 `keyenvRevision`/`keyenvHeader`/`keyenvVaultFolder` 专用标记，拒绝把实验文件夹或公开 fixture key 当正式库。OAuth 改为 online 请求，token 只留进程内，不保存 refresh token；真实运行每次仍需用户浏览器授权。

实现边界：沿用原 Argon2id/AES-256-GCM primitive 与封装；初始化和导出不覆盖，原子完整文件发布；绑定信息加密验证；已有库持进程锁。交互更新提交前复核所见 heads，拒绝看不见的并发变更。显式 resolution 字段允许用户选择版本合并同名分支，并保留所有原始对象；旧原型不保证读取该新增字段的兼容性。初始化先持久化预生成 folder/header ID，再创建远端对象；响应丢失复用同 ID 校验恢复，不另建替代库。dotenv 对可能被不同加载器改变的字符串安全拒绝，任意字符串改用 JSON 或直接环境注入。

**本次实际通过**（Mac arm64、Go 1.27.1、CGO_ENABLED=0）：全套 `go test -count=1 ./...`、`go vet ./...`、`cmd/keyenv` 与 `cmd/keyenv-lab` 两个 Mach-O arm64 构建、simulate 六项 true、正式 CLI 非终端 init 返回 `INTERACTION_REQUIRED` 且不创建目标目录、`git diff --check`。CI 增加正式 CLI 构建，但未推送或运行 GitHub CI。

新增自动测试证据：初始化/备份/恢复/导出拒绝覆盖；错误密码及解锁失败后释放锁；0600/0700 与公开文件拒绝；主密码、合成凭据名/值和绑定账号/应用不在磁盘明文出现；备份脱离 Drive 恢复且更新 device ID；待上传跨进程恢复、同 ID 重试不重复；损坏本机缓存/绑定不作为离线数据；NUL 拒绝；JSON 字面值往返、dotenv 歧义拒绝；参数/环境字面传递、退出码 17 和信号退出码 143；交互编辑远端变化拒绝；同名显式解决保留所有历史；production/lab Drive 标记隔离、账号查询和唯一 header 校验；folder 创建响应丢失后同 ID 409 恢复。HTTP 测试均是合成服务器，不是真实 Google。

**OAuth 当前真实观察**：Chrome 可操作，2SV 门槛已解除；已有 Default Gemini Project 没有 OAuth client。已创建专用项目 `keyenv-personal`（ID 同名），已启用 Google Drive API。应用表单准备为 Keyenv Personal、External/Testing、支持/联系邮箱 rentlifelee@gmail.com，停在新增 Google API 服务用户数据政策复选框等待当次确认。尚未提交 OAuth 应用配置、创建 Desktop client、下载 client JSON 或授予 Drive 访问；未开试用或计费。

**未验收/下一步**：接受新 API 用户数据政策后完成 consent、test user、drive.file 数据范围及 Desktop client；凭据创建和真实授权依规定取得当次确认，秘密由用户本地输入。运行两次真实 drive-probe，再用户亲自执行正式 setup。完整终端 add/update/run/export 与信号转发/交互子进程实机体验、第二台 Mac、真实 Drive 一致性/撤权/分页、格式独立评审、插件包装/安装器仍待完成。本次没安装 Xcode，未跑可选 race。此检查点不是正式产品已验收。

## 本地 Mac 接续检查（2026-10-12）

- 实际环境：macOS 26.5.2（25F84）、arm64；本地已安装 Google Chrome。
- 未发现已有 personal-plugins checkout，因此在独立工作目录 clone。`main` 与 `origin/main` 均为 `3967a7a`，包含手动密码解锁变更；clone 后工作区干净，没有覆盖或重置用户修改。
- 仓库中没有 `AGENTS.md` 或 `.agents/skills`；已阅读本地交接、设计、验证和工具 README。
- 本机 PATH 及检查的常见工具目录没有 Go。官方发布元数据可读取，选择的 Go 1.27.2 darwin-arm64 归档 SHA-256 为 `76812b213b1b2302c978d28fa52fa92d541704b9e7d9d5db8002c50e4018c4c5`，但尚未下载成功或安装。
- 下载阻塞：go.dev 跳转到 dl.google.com 后 curl TLS 连接失败，直接重试同样失败；官方 storage.googleapis.com/golang 路径返回 HTTP 403。Python TLS 栈也因缺少可信签发证书失败。没有关闭证书验证、使用第三方镜像或安装 Xcode。
- 下载诊断：系统 UTC 时间为 2026-10-11，系统代理及 HTTP(S)_PROXY/ALL_PROXY 均未设置。获网络审批后 dl.google.com 可解析并建立 TCP，但在 TLS ClientHello 后失败，没有 HTTP 状态；不是自动审批拒绝。Python 默认 CA 文件不存在，另有证书信任环境问题。所有尝试均保留 TLS 校验。
- 恢复：Homebrew 官方 formula 元数据可访问，`brew install --force-bottle go` 成功。其本机 formula 版本为 Go 1.27.1（满足 1.26+），安装 arm64_tahoe 预编译 bottle，无运行依赖；未安装 Xcode。Go 编译器与 CGO/C 工具链是不同依赖，移除 CGO 不会移除编译 Go 源码的需求。GitHub releases API 返回空列表，未发现仓库发布的预编译产物。
- **本次 Mac 通过**：`CGO_ENABLED=0 go test -count=1 ./...`、`CGO_ENABLED=0 go vet ./...`、无 CGO 构建；产物是 Mach-O arm64。`simulate` 六项均为 true；真实本机进程重定向 stdin 返回 `INTERACTION_REQUIRED`（退出码 1）。这是自动化/非交互证据，不能替代用户隐藏输入体验。
- 已准备仓库外专用交互脚本，并通过 `zsh -n`；已在本地 Terminal 打开，等待用户亲自验证不回显、确认不一致、独立进程正确/错误密码、拒绝覆盖及 0600 权限。脚本只保存脱敏状态并清理明确 fixture；不记录输入、不开启 shell trace。
- 后续状态复核：Go 1.27.1 位于 `/opt/homebrew/Cellar/go/1.27.1`，`/opt/homebrew/bin/go` 已在当前 PATH；实验 binary 位于 `tools/keyenv/bin/keyenv-lab`，未安装为全局 `keyenv-lab` 命令。无需重复安装。实验进程仍运行，0600 脱敏结果文件仅有首个隐藏输入提示、没有验证结果，当前等待用户第一步操作；未重启或读取输入。读取 Terminal 窗口标题的 AppleScript 未返回，已取消，不据此猜测确切系统窗口标题。
- **未运行**：真实用户手动密码验证、Google OAuth、实际 Drive 和双 Mac 测试；可选 race 未运行，未为其安装 C 工具链。
- 下一步由用户完成已打开的本地 Terminal 隐藏输入实验，再使用 Chrome 配置 OAuth。未创建 Google Cloud 项目或 OAuth client，未授权任何权限，也未生成密码 fixture 或 OAuth token。

### 用户调整后的手动实验与 OAuth 状态

用户随后表示已操作，并明确要求跳过剩余手动密码验证，不追加或重复实验。现有脱敏日志只证明 `password-probe create` 成功，输出 `synthetic_data_only=true` 和 `unlock_material_persisted=false`。脚本首步标签为 mismatch，但实际退出码为 0 并创建文件，因此不能将确认不一致记为通过。独立进程正确/错误密码、实际不回显、拒绝覆盖和 fixture 权限的用户体验检查均记为**用户要求跳过**，不是失败或通过。明确的仓库外测试 fixture 已删除；不读取密码或输入历史，不重启交互窗口。

已开始 Google OAuth 准备：本地 Chrome 页面确认登录 `rentlifelee@gmail.com`；浏览器扩展连接失败，改用本机 UI 控制。Downloads 和预期 keyenv 配置目录未找到 Desktop OAuth client JSON（仅记录数量，不展示内容）。Google Cloud 首次使用显示服务条款弹窗、国家/地区当前为新加坡；停在接受条款之前，等待用户亲自处理或当次明确确认。未启动免费试用、设置计费、创建项目或 OAuth credentials，也未请求 Drive 权限。后续原型权限仍限定 `drive.file`，token 仅存进程内，真实 Drive 与双 Mac 验证仍未运行。

用户阅读具体条款链接并当次明确确认后，已接受 Google Cloud 条款，保留新加坡地区、营销邮件未勾选。页面确认已进入控制台。全部项目列表只有 `Default Gemini Project`（`gen-lang-client-0278756149`）；仅选择它作只读检查，没有修改 Gemini 项目配置。进入 OAuth 凭据列表被重定向至 Cloud 的 MFA 门槛页面，显示账号必须启用两步验证才能继续。已有 OAuth client 尚未能核实。认证设置更改交由用户亲自在本地 Chrome 完成；不读取或代填密码、验证码、备份码。当前未创建项目或 credentials、未启用 Drive API、未进行 Google OAuth 授权、未开通试用或计费。

### 等待账号验证期间的原型修正

交接要求先完成探针验证及阻塞修复，正式 setup/add/run/export 是明确尚未完成的后续功能。本次没有新增正式初始化命令，也没有生成用户正式 vault。正式本地初始化的后续最小计划：复用 `vault.Create` 与现有隐藏输入；选择仓库外私有目录；独占创建、不得覆盖既有 vault；原子保存现有格式 header 和初始密文状态；独立进程解锁并验证损坏/错误密码/权限边界。Drive 接入时需另行绑定明确 vault/folder/account/application，不能把固定 fixture key 当真实数据密钥。开始这项后续实现前应明确正式初始化范围与格式评审要求。

本次实际改动：Drive probe 在读取 OAuth 配置或授权前显示“fixture 密钥公开、只上传内置虚构数据、不使用主密码或初始化正式库”；成功 JSON 增加 `fixture_key_public=true`、`master_password_used=false`。README 对应说明没有用户数据导入参数，禁止将固定 fixture 改成真实凭据。已有文件夹检查保持原行为；新增 HTTP 合成测试覆盖专用标记、普通文件夹、错误标记、标记为实验的普通文件、已删除文件夹和损坏元数据，并确保检查只读取所选文件夹元数据。

修改后 Mac Go 1.27.1 实际通过无 CGO 全部单元/HTTP 测试、vet、构建、simulate（六项 true）。使用不存在的 OAuth 配置路径做无网络入口检查，确认先显示公开密钥提醒、随后返回 `AUTH_REQUIRED`（退出码 1）；未发生浏览器授权或远端写入。上述是模拟及本机无账号检查，真实 Drive/OAuth、双 Mac 验收仍待用户两步验证及后续授权。


## 环境与范围

本轮在 Linux amd64 云端工作区运行。使用 Go 官方发布元数据指定的 SHA-256 校验安装的 Go 1.27.2；模块最低 Go 1.26，依赖版本固定在 go.mod/go.sum。

Google Drive 通过 HTTP 测试服务器验证适配逻辑，双客户端同步通过内存故障存储验证；没有登录用户 Google 账号，没有创建远端测试文件，没有导入真实 token。

实际执行结果：`go test -race -count=1 ./...`、`go vet ./...`、构建与 `keyenv-lab simulate` 全部通过。`FuzzOpen` 进行了 120,053 次输入实验并通过；这只是有界短时模糊测试，不等于密码学审计。离线模拟输出的六项检查均为 true。

## 已通过的自动化场景

- 正确/错误主密码、密文篡改、对象 ID / vault ID 替换、未知格式、KDF 参数与输入大小上限。
- 两个客户端并发更新同条目保留双方，冲突阻止选用；不同条目并发可共存，同名新条目不能静默合并。
- 停用阻止选用；多父版本能表示显式解决冲突；缺失父版本、循环和跨条目父关系被拒绝。
- 密文缓存可离线读取，新离线修改被拒绝；认证过期可使用已有缓存，损坏对象不能触发离线回退。
- 上传成功但响应丢失后，待上传记录仍保留；重启后同 ID 重试不会新增重复对象。
- 未确认上传的修改不会作为默认运行版本；保存本机状态失败时不会上传。
- Drive multipart 编码、分页、409 后校验相同内容、拒绝不同内容、拒绝其他文件夹对象。
- Drive 部分页失败、重复分页 token、重复对象、incompleteSearch、过大响应及 HTTP 错误分类。
- 本地密文状态原子替换、0600 文件权限、拒绝公共可读目录或缓存文件。
- 脱敏摘要不包含测试秘密字段值；持久化状态不包含条目名称或秘密原文。
- 原钥匙串接口已移除；新的手动密码验证结果见下方变更记录。

## 尚未验证

1. macOS 实机隐藏输入、错误密码和跨进程重新解锁体验。
2. 同一 Gmail 与同一个 OAuth 应用在两台 Mac 的实际 drive.file 可见性、Google 实际预生成 ID 上传及 409 行为。
3. Google OAuth 跨周刷新：原型只维持进程内登录，不持久化刷新令牌。
4. 实际 Drive 延迟可见性和账号撤权；自动测试的错误模拟不能替代真机结果。
5. 正式 CLI 的环境变量注入、文件导出、进程锁、信号传递、备份恢复和安装体验：这些功能尚未实现。
6. 密码格式独立评审、长期兼容性、主密码变更和完整设备撤权。

## 复现

在 `tools/keyenv` 内运行：

```sh
CGO_ENABLED=0 go test -count=1 ./...
go vet ./...
CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
```

macOS 与 Google 实机步骤见 [实验工具 README](../tools/keyenv/README.md)。返回结果只需包含是否成功、错误代码、系统版本和是否出现授权提示，不要附 token、主密码或 OAuth client 文件内容。

## 手动主密码变更验证

已移除整个原生钥匙串包及 `keychain-probe`，替换为 `password-probe create/check --file PATH`。当前不持久保存主密码、数据密钥或 OAuth token，没有跨进程自动解锁。

本次在 Linux amd64、Go 1.27.2 实际通过：

- `CGO_ENABLED=0 go test -count=1 ./...`、`go test -race -count=1 ./...`、`go vet ./...`。
- 无 CGO 本机构建，`simulate` 六项均为 true。
- `CGO_ENABLED=0 GOOS=darwin GOARCH=arm64` 及 `amd64` 交叉构建；不等于已在 Mac 运行。
- 密码 fixture 单元测试：正确密码、错误密码、对象绑定篡改、尾随 JSON 拒绝，序列化内容不含测试密码或合成明文。
- Linux 伪终端端到端实验：输入不回显、确认不一致不写文件、新进程重新解锁、错误密码拒绝、文件权限 0600、已有文件拒绝覆盖、管道输入返回 `INTERACTION_REQUIRED`。仅使用公开合成测试密码，临时文件已清理。

Mac 实际终端体验、Google OAuth、真实 Drive 及双 Mac 可见性仍未执行。密码实验和 Drive fixture 实验各自独立，尚未实现完整的手动主密码凭据库产品流程。CI 配置已增加无 CGO 测试及构建；此记录不代表 GitHub CI 已完成。

### Follow-up security and packaging checkpoint

- Read-only review identified hidden-input signal restoration and duplicate child signal forwarding; both corrected. `run` now closes the vault session and replaces the CLI process with the target using `exec`, retaining native terminal signals and exit status.
- Synthetic PTY tests on this Mac verified SIGINT/SIGTERM during hidden input restore ECHO, return 130/143, and create no vault. These are automated synthetic tests, not user-password acceptance.
- Fresh `CGO_ENABLED=0 go test -count=1 ./...` passed with authorized loopback access; sandbox-only run could not bind httptest's local port. `go vet`, build, and whitespace validation passed.
- Import initialization failure now removes only the newly created empty directory.
- Added host-local build package and explicit-destination installer, tested in a temporary directory including overwrite refusal. No global install or shell configuration modification performed. Checksums detect accidental corruption, not publisher authenticity.
- Added portable `plugins/keyenv/skills/keyenv/SKILL.md`; native plugin registration is not claimed.
- Google API user-data policy acceptance remains pending. No OAuth Desktop client, credential JSON, token grant, real Drive sync or real user vault initialization completed at this checkpoint.
- Offline cache freshness and complete rollback detection are outside current guarantees. Byte-slice clearing is not a guarantee of complete Go process-memory erasure.

### OAuth policy checkpoint

User explicitly confirmed the Google API Services User Data Policy through the parent thread. After Air was unlocked, accepted that policy in the existing Chrome `keyenv-personal` configuration flow and saved the prepared application branding. Google Cloud displayed “已创建 OAuth 配置！” and the overview explicitly showed no OAuth clients configured. Opened the client creation form without submitting. Desktop credential generation, credential download, test-user/scope access changes and actual Drive authorization remain pending their specific action-time confirmation. No billing/trial activated and no secret read or entered.

### Desktop client creation incident checkpoint

The user approved creating the Desktop client and saving its JSON outside Git. Created `Keyenv Local Desktop` in `keyenv-personal`. During UI control discovery, filtering for button labels inadvertently included Google's copy-secret accessibility button label, exposing the client secret in tool output. No credential JSON was downloaded, no Drive grant obtained, and the credential was not used. Closed the secret dialog. The user was informed and subsequently explicitly chose to retain this client. That decision supersedes the earlier proposed deletion/recreation; do not delete or recreate it. The new client secret itself is deliberately not recorded here.
