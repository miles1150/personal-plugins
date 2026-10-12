# Keyenv laboratory prototype

这是第一轮技术验证工具，使用合成测试数据。它不是正式 `keyenv`，不应导入真实 API key/token；加密格式尚未定稿，之后可能不兼容。

## 已实现

- Argon2id 主密码派生、随机数据密钥封装、AES-256-GCM 对象加密，以及格式和资源上限检查。
- 不可变版本图：保留不同条目修改，检测同条目与同名新增冲突，停用后拒绝选择；图算法可验证多父版本的冲突解决。
- 上传前保存密文、重启恢复、上传结果不明时的同 ID 重试、离线只读缓存。
- Google Drive HTTP 适配器：分页、预生成 ID、multipart 上传、409 后内容核验、文件夹归属检查和响应大小限制。
- 终端隐藏输入主密码，解锁材料只保留在当前进程内；不依赖系统钥匙串。
- `keyenv-lab simulate`、`drive-probe`、`password-probe` 三个验证入口。

正式 `cmd/keyenv` 已有首轮开发实现，使用方法和当前限制见下方。`keyenv-lab` 仍为独立技术探针。尚未完成插件包装、完整安装器和持续 OAuth 登录；真实 Google Drive、正式用户库与完整用户流程尚未验收。

## 正式 CLI 首轮开发版本（待实机验收）

构建：`CGO_ENABLED=0 go build -o bin/keyenv ./cmd/keyenv`。用户运行构建后的 binary 不需要 Go 编译器或 Xcode。尚未安装到全局 PATH。

全局 `--home` 必须放在命令前，路径为新建的仓库外绝对目录；父目录须已存在。所有解密命令由用户亲自在终端隐藏输入主密码。无终端返回 `INTERACTION_REQUIRED`，不从参数、环境变量、聊天或钥匙串获取密码。

```sh
# 可单独建立空本地库；这不启用离线编辑，也不连接 Google
./bin/keyenv --home /absolute/private/keyenv init
# 首台设备：交互确认，仅申请 drive.file；OAuth token 只存内存
./bin/keyenv --home /absolute/private/keyenv setup --oauth-client /absolute/private/client.json --account rentlifelee@gmail.com
# 其他设备：必须提供明确已有 folder ID，不按名称自动发现或新建
./bin/keyenv --home /absolute/private/keyenv setup --oauth-client /absolute/private/client.json --account rentlifelee@gmail.com --folder FOLDER_ID
./bin/keyenv --home /absolute/private/keyenv add openai
./bin/keyenv --home /absolute/private/keyenv update openai
./bin/keyenv --home /absolute/private/keyenv list --json
./bin/keyenv --home /absolute/private/keyenv inspect openai
./bin/keyenv --home /absolute/private/keyenv run --credential openai -- python app.py
./bin/keyenv --home /absolute/private/keyenv export --credential openai --format json --output /absolute/private/selected.json
./bin/keyenv --home /absolute/private/keyenv sync
./bin/keyenv --home /absolute/private/keyenv disable openai
./bin/keyenv --home /absolute/private/keyenv resolve openai
./bin/keyenv --home /absolute/private/keyenv backup --output /absolute/private/encrypted-backup.json
./bin/keyenv --home /absolute/private/restored-keyenv restore --input /absolute/private/encrypted-backup.json
```

`status` 本地检查；`doctor` 检查当前 Drive 绑定，不测试凭据服务商 token。`list`/`inspect`/`run`/`export` 可加 `--offline` 显式读取已验证缓存；缓存为空或损坏不能回退使用。修改必须在线成功拉取；待上传版本不作为默认运行版本。每次联网命令需重新浏览器授权，不持久化 OAuth token。

本地 `vault.json` 是 0600 文件，包含已有格式的 wrapped-key header、加密的账号/应用/库绑定与密文 revision 状态。主密码和未封装的数据密钥不保存。macOS/Linux 使用进程文件锁；子进程启动前释放锁。状态输出包含最近成功拉取时间、待上传数量及缓存使用标记，不代表全球即时一致。初始化会保存预生成 folder/header ID，失败重试复用它们，避免静默生成替代库。

更新输入保留未提及的字段；用途、到期日是脱敏元数据，不能填写秘密。到期日只提示，不代表服务商撤销。`resolve` 展示 head/entry/device ID 和字段名，用户明确选择一个版本；多父合并保留全部密文历史，同名不同 ID 也仅在显式确认时合并。加密备份恢复到新目录，原 ID/密文保持、设备 ID 更新；不会覆盖已有目录，未确认上传仍保持待处理。

`run` 使用参数数组启动程序，秘密作为字面环境变量注入；保持子进程输出，不能保证子进程不打印或发送秘密。工具状态写 stderr，只报告覆盖字段名。导出需用户交互确认，0600 原子创建且不覆盖；JSON 支持任意合法字符串。dotenv 使用单引号简易字面格式，对单引号、反斜线、美元符、控制字符等有解析歧义的值拒绝导出，应使用 JSON 或 `run`；不得 source 导出文件。

当前验收缺口：真实 setup/OAuth/Drive、跨设备库接入、完整终端凭据录入、转发信号和交互子进程体验、长期格式独立评审、安装/插件包装。开发测试只用合成数据，不能因此宣布生产可用。不要向 agent 提供正式密码，也不要在旧 password-probe 窗口输入正式密码。

## 构建与自动检查

需要 Go 1.26 或更新版本。程序为纯 Go，正常构建不需要 Xcode Command Line Tools、C 编译器或 macOS SDK。`-race` 是可选开发检查，需要 CGO 和 C 工具链，不是运行或普通测试的前提。

从本目录执行：

```sh
CGO_ENABLED=0 go test -count=1 ./...
go vet ./...
CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
```

`simulate` 完全离线，使用内存存储模拟两台设备。输出每项故障实验的布尔结果，不输出测试 token。

## 手动主密码验证

在你自己的交互终端执行；使用专门的测试密码，不要输入真实 API token：

```sh
./bin/keyenv-lab password-probe create --file /absolute/private/path/password-fixture.json
./bin/keyenv-lab password-probe check --file /absolute/private/path/password-fixture.json
```

`create` 隐藏输入并确认主密码，以 0600 权限创建测试文件，拒绝覆盖已有文件。文件仅包含 KDF 参数、封装后的随机数据密钥和加密的合成数据。`check` 在新进程重新输入密码并验证，成功输出 `unlock_material_persisted: false`。密码错误返回 `DECRYPTION_FAILED`；输入不一致返回 `PASSWORD_MISMATCH`。重定向 stdin、管道或没有交互终端时返回 `INTERACTION_REQUIRED`，不从环境变量或命令参数读取主密码。

主密码由你自行保管，程序不记住它。没有跨命令解锁会话或后台进程；需要解密的独立命令每次都要输入密码。忘记密码后无法靠 Google 登录恢复。代码尽力清理持有的秘密字节，但 Go 运行时不保证所有内存副本被擦除。

测试完后手动删除该测试文件。此实验验证本地加密与手动解锁；下方 Drive 实验独立使用公开 fixture 密钥，不代表二者已经集成为正式凭据库。

## Google Drive 双客户端验证

### 一次配置

1. 使用你的个人 Google Cloud 项目，启用 Google Drive API。
2. 配置 Google Auth Platform 的 audience / consent，并创建 **Desktop app** OAuth client。
3. 下载 client JSON，保存在仓库之外。它不是 API key，也不应该粘贴到聊天或提交 Git。
4. 原型只申请 `https://www.googleapis.com/auth/drive.file`。测试期间若 audience 为 External/Testing，需要将自己的 Gmail 添加为 test user。

长期使用不能依赖 Testing 的令牌生命周期：带 Drive scope 的刷新令牌通常 7 天后过期。转入适合自用的生产发布状态后重新授权，是后续 setup 的一部分；生产状态、应用验证和令牌永久有效是不同概念。**本原型只在进程内持有 OAuth token，不存储登录，所以每次运行都会经过授权。它不验证跨周刷新。**

### 第一台 Mac

```sh
./bin/keyenv-lab drive-probe --oauth-client /absolute/path/client.json
```

打开终端显示的 URL，在这台 Mac 的浏览器中授权。回调仅监听本机 127.0.0.1，使用随机 state 和 PKCE。

命令创建 `Keyenv Lab — test data only` 文件夹，在其中写入一份加密的固定虚构字符串，并重复上传同一个预生成 ID 以验证幂等行为。测试加密密钥是源码中公开的固定 fixture，**只用于虚构数据**。它不修改文件夹之外的文件，也不会自动删除测试文件。

授权前程序会明确提醒 fixture 密钥公开，成功 JSON 标注 `fixture_key_public: true` 和 `master_password_used: false`。没有导入数据或录入真实 token 的参数；不要修改固定 fixture 来存放真实凭据。此入口不会设置正式主密码或初始化正式 vault。指定已有文件夹时，必须具有原型专用标记、正确文件夹类型且未被删除，否则拒绝继续。

返回的 JSON 包含 `folder_id`、`created_file_id`、已读到的 fixture 数量与幂等结果。若后续操作失败，已创建的 folder ID 也会显示在 stderr，方便清理。

### 第二台 Mac

使用同一份 OAuth client 配置、同一 Gmail，执行：

```sh
./bin/keyenv-lab drive-probe --oauth-client /absolute/path/client.json --folder FOLDER_ID
```

它检查明确指定的测试文件夹、读取并解密已有 fixture，然后新增一个测试对象。确认 `existing_fixtures_verified` 至少为 1。再回第一台 Mac 用相同 folder ID 运行，检查是否能读到第二台写入的数据。

这是传输和跨设备可见性验证，不是生产 vault 的 setup；网络中断后的持久重试目前由自动化同步测试验证，probe 本身不保存待上传队列。网络故障可能留下一个已成功写入的 fixture，不代表数据损坏。

不要通过更换 OAuth 应用、增加整盘 scope 或复制 refresh token 来掩盖跨设备发现失败，应先记录结果。测试完后，在 Drive 手动删除这个明确标注的测试文件夹即可。

## 当前验证状态

- Linux 自动化测试、race 检查、静态检查、构建和离线模拟：见 [验证记录](../../docs/keyenv-validation.md)。
- macOS 无 CGO 交叉构建与本机交互终端验证：见验证记录；Mac 实机仍待执行。
- Google OAuth、两台 Mac 的 Drive 访问及真实 409 行为：待执行。
- 仓库提供 Ubuntu/macOS CI 工作流，但添加工作流不代表已经在 GitHub 运行。

Local packaging: `./scripts/build-local.sh` creates a host package under ignored `dist/`. To install into an explicitly selected existing bin directory, run `./scripts/install-local.sh ABSOLUTE_PACKAGE_DIR ABSOLUTE_BIN_DIR`. The installer verifies the local checksum, refuses existing files, and leaves PATH and shell configuration unchanged. Packages are local build artifacts, not signed releases. A portable assistant skill source is at `../../plugins/keyenv/skills/keyenv/SKILL.md`; it has not been registered as a native plugin.
