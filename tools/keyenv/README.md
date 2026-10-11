# Keyenv laboratory prototype

这是第一轮技术验证工具，使用合成测试数据。它不是正式 `keyenv`，不应导入真实 API key/token；加密格式尚未定稿，之后可能不兼容。

## 已实现

- Argon2id 主密码派生、随机数据密钥封装、AES-256-GCM 对象加密，以及格式和资源上限检查。
- 不可变版本图：保留不同条目修改，检测同条目与同名新增冲突，停用后拒绝选择；图算法可验证多父版本的冲突解决。
- 上传前保存密文、重启恢复、上传结果不明时的同 ID 重试、离线只读缓存。
- Google Drive HTTP 适配器：分页、预生成 ID、multipart 上传、409 后内容核验、文件夹归属检查和响应大小限制。
- 终端隐藏输入主密码，解锁材料只保留在当前进程内；不依赖系统钥匙串。
- `keyenv-lab simulate`、`drive-probe`、`password-probe` 三个验证入口。

尚未实现正式 CLI 的 setup/add/run/export 等产品命令、插件包装、完整安装器、持续 OAuth 登录、备份恢复界面和进程锁。同步引擎当前约定单进程持有它，不能直接当作可并发运行的生产客户端。未验证真实 Google Drive 一致性及 macOS 行为。

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
