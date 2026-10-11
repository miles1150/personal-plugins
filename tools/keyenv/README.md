# Keyenv laboratory prototype

这是第一轮技术验证工具，使用合成测试数据。它不是正式 `keyenv`，不应导入真实 API key/token；加密格式尚未定稿，之后可能不兼容。

## 已实现

- Argon2id 主密码派生、随机数据密钥封装、AES-256-GCM 对象加密，以及格式和资源上限检查。
- 不可变版本图：保留不同条目修改，检测同条目与同名新增冲突，停用后拒绝选择；图算法可验证多父版本的冲突解决。
- 上传前保存密文、重启恢复、上传结果不明时的同 ID 重试、离线只读缓存。
- Google Drive HTTP 适配器：分页、预生成 ID、multipart 上传、409 后内容核验、文件夹归属检查和响应大小限制。
- 原生 macOS 本地 login Keychain 接口，经 Security framework 传值，不把秘密放入 `security` 命令参数。
- `keyenv-lab simulate`、`drive-probe`、`keychain-probe` 三个验证入口。

尚未实现正式 CLI 的 setup/add/run/export 等产品命令、插件包装、完整安装器、持续 OAuth 登录、备份恢复界面和进程锁。同步引擎当前约定单进程持有它，不能直接当作可并发运行的生产客户端。未验证真实 Google Drive 一致性及 macOS 行为。

## 构建与自动检查

需要 Go 1.26 或更新版本。Mac 上还需要 Xcode Command Line Tools 提供 C 编译器与 SDK，以构建原生钥匙串接口。

从本目录执行：

```sh
go test -race -count=1 ./...
go vet ./...
go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
```

`simulate` 完全离线，使用内存存储模拟两台设备。输出每项故障实验的布尔结果，不输出测试 token。

## Mac 本机钥匙串验证

```sh
./bin/keyenv-lab keychain-probe create
```

它会在 `dev.personal-plugins.keyenv-lab` 服务下创建一个随机测试值，返回 `probe_id` 和该高熵随机值的 SHA-256 指纹。它不会读取你的已有 API key。

系统可能提示批准钥匙串访问。用返回的 ID 替换以下示例中的 `PROBE_ID`：

```sh
./bin/keyenv-lab keychain-probe check --id PROBE_ID
```

然后分别执行重新登录、锁屏/解锁、重新编译同一个路径的二进制，再运行 `check`，记录是否额外提示、指纹是否一致。锁屏或授权拒绝时失败是需要记录的行为，不能解释成秘密丢失。

清理本次测试值：

```sh
./bin/keyenv-lab keychain-probe delete --id PROBE_ID
```

`create` 到 `delete` 之间会保留这个测试条目。创建后第一次读取若被拒绝，条目仍可能存在，可在 Keychain Access 中按上述服务名检查和清理。未提供无需开发者账号的稳定发布签名方案；升级后的提示频率是这个原型要验证的问题。

非 macOS 或关闭 CGO 的构建返回 `MACOS_KEYCHAIN_REQUIRED`，不会退回明文文件存储。

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
- 原生 macOS 编译及登录钥匙串实机验证：待执行。
- Google OAuth、两台 Mac 的 Drive 访问及真实 409 行为：待执行。
- 仓库提供 Ubuntu/macOS CI 工作流，但添加工作流不代表已经在 GitHub 运行。
