# Keyenv 开发版本

仓库同时包含正式 CLI `keyenv` 和只使用公开合成数据的实验工具 `keyenv-lab`。当前格式尚未完成独立评审，不能把实验工具用作真实凭据库。

## 已实现

- Argon2id 主密码派生、随机数据密钥封装、AES-256-GCM 对象加密，以及格式和资源上限检查。
- 不可变版本图：保留不同条目修改，检测同条目与同名新增冲突，停用后拒绝选择；图算法可验证多父版本的冲突解决。
- 上传前保存密文、重启恢复、上传结果不明时的同 ID 重试、离线只读缓存。
- Google Drive HTTP 适配器：分页、预生成 ID、multipart 上传、409 后内容核验、文件夹归属检查和响应大小限制。
- setup 隐藏输入一次主密码，缓存本机解锁密钥；主密码不落盘，不依赖系统钥匙串。后续维护不再询问主密码；提供 `unlock`／`lock`。
- `keyenv-lab simulate`、`drive-probe`、`password-probe` 三个验证入口。

正式 `cmd/keyenv` 已有首轮开发实现，使用方法和当前限制见下方。`keyenv-lab` 仍为独立技术探针。加密的本机 OAuth 缓存及自动刷新已实现；插件包装、完整安装器及跨周登录验收尚未完成；本机一次真实持久授权、跨进程复用及即时刷新已通过；本机真实 OAuth、Drive 上传/读回/分页、正式库 setup、公开条目录入和加密备份/离线恢复已通过；完整状态见 [验证记录](../../docs/keyenv-validation.md)。跨 Mac 和实际服务认证仍待验收；当前五个真实服务字段已由本人隐藏录入，并通过新登录 shell／子进程存在且非空校验。

## Git 管理公开客户端配置与加密凭据

本次用户明确选择：桌面 OAuth 应用的公开字段明文维护，client_secret 加密后通过 Git 分发。仓库中的 `config/oauth/client.public.json` 只含白名单 client_id、project_id、auth_uri、token_uri、auth_provider_x509_cert_url、redirect_uris；`config/oauth/client.secret.enc.json` 含 AES-GCM 密文、公开配置 SHA-256 和封装后的库密钥头。它不包含明文数据密钥、主密码、OAuth token、解锁缓存或服务密钥。未知字段（包括任何 token）拒绝公开。两文件绑定版本、应用 ID、公开配置摘要、库 ID 和原库头摘要，错误密码、换文件、改配置或密文会拒绝解密。Google 授权/令牌端点仍固定为官方地址，不采信仓库中的任意端点。

本机已解锁的现有库可以打包，无需再次输入主密码或 Google 登录，不修改原客户端或库：

```sh
./bin/keyenv --home /absolute/private/keyenv client-pack --oauth-client /absolute/private/oauth/client.json --output /absolute/path/NEW_BUNDLE_DIRECTORY
```

输出目录必须不存在，已有文件不覆盖。只有公开配置与密文可以提交 Git；完整 client.json 被忽略，并拒绝解密输出到 Git checkout（包括符号链接别名）。新设备从 Git 获取配置后，用原主密码在终端隐藏输入一次，解密到仓库外 0600 文件，然后完成本人浏览器授权；同一次输入用于库解锁。所选 Drive 文件夹的真实 header 必须与密文携带的 header 完全一致，不另建库或客户端。`--oauth-client` 可指定完整文件生成位置；省略时为本机库父目录下 oauth/client.json。父目录须已存在，私密目录 0700。已有相同配置幂等复用；不同配置、公开权限、符号链接拒绝，不自动替换。

```sh
./bin/keyenv --home /absolute/private/keyenv setup --oauth-bundle /absolute/repo/tools/keyenv/config/oauth --account rentlifelee@gmail.com --folder EXISTING_FOLDER_ID
```

也可从仓库根目录执行中文助手（账户和文件夹 ID 是非秘密参数）：

```sh
/bin/sh tools/keyenv/scripts/connect-existing.sh rentlifelee@gmail.com EXISTING_FOLDER_ID
```

脚本无 CGO 编译最新正式 CLI，默认使用 `$HOME/.config/keyenv/vault`，新设备首次接入后安装 .zprofile 并仅列字段名；已有本机库只 sync，不重跑 setup。不复制其他机器的 unlock-cache.json/oauth-login.json。agent 仍需使用登录 zsh 才能继承字段。本机真实客户端已打包并用现有解锁 key 验证密文回读；新设备本人主密码解密和 Google 授权仍待 Air 实际完成，不把合成测试算成双机验收。

配置更新时重新打包匹配的两文件；公开配置不能脱离密文单独修改。若以后改变库 header/主密码或客户端 secret，也须重新生成对应包，Git 历史里的旧密文仍存在。新设备只在本人隐藏输入时使用主密码，不保存它。当前只分发应用客户端凭据，OAuth 授权和本机缓存仍分别建立。

## 正式 CLI 开发版本

构建：`CGO_ENABLED=0 go build -o bin/keyenv ./cmd/keyenv`。用户运行构建后的 binary 不需要 Go 编译器或 Xcode。尚未安装到全局 PATH。

全局 `--home` 必须放在命令前，路径为新建的仓库外绝对目录；父目录须已存在。首次 setup 或新设备 restore 由用户亲自在终端隐藏输入一次主密码，随后自动保存本机解锁密钥。已有旧库执行一次 `unlock` 即可启用；维护命令直接读取缓存，无缓存时返回 `LOCAL_UNLOCK_REQUIRED`，不自动弹出密码输入。主密码只在本人终端输入，不从参数、环境变量、聊天或钥匙串获取。

```sh
# 可单独建立空本地库；这不启用离线编辑，也不连接 Google
./bin/keyenv --home /absolute/private/keyenv init
# 首台设备：交互确认，仅申请 drive.file；OAuth token 加密保存在本机
./bin/keyenv --home /absolute/private/keyenv setup --oauth-client /absolute/private/client.json --account rentlifelee@gmail.com
# 其他设备：必须提供明确已有 folder ID，不按名称自动发现或新建
./bin/keyenv --home /absolute/private/keyenv setup --oauth-client /absolute/private/client.json --account rentlifelee@gmail.com --folder FOLDER_ID
# 旧版本已建立的库：本人一次隐藏输入后启用缓存，不重做 setup 或 Google 登录
./bin/keyenv --home /absolute/private/keyenv unlock
# 显式删除本机解锁缓存（需要时执行）
# ./bin/keyenv --home /absolute/private/keyenv lock
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

`setup` 是首次配置／新设备接入步骤，与条目维护分开。正常 setup 只输入一次主密码，不重复确认；本地 `init` 保留首次密码确认。已有库的 `add`、`update`、`run`、`backup` 不会调用 setup 或自动另建 Drive 文件夹。首次本人浏览器授权后，本机加密保存 access/refresh token，后续联网维护复用并自动刷新；令牌失效时明确返回 AUTH_REQUIRED，由本人 login --reauthorize，不自动反复打开浏览器。离线备份恢复无需 Google 登录。

提示使用中文。单字段维护可预设字段名和非秘密元数据，跳过用途、到期日、数量和名称的问题，字段值仍由本人隐藏输入：

```sh
./bin/keyenv --home /absolute/private/keyenv add --field OPENAI_API_KEY --purpose 模型调用 openai
./bin/keyenv --home /absolute/private/keyenv update --field OPENAI_API_KEY openai
# 已缓存解锁密钥后，无需输入主密码，写密文备份并恢复到新目录，验证全部解密版本、
# 待上传密文和绑定保持一致，设备 ID 更新；结果不含字段值，不访问 Drive。
./bin/keyenv --home /absolute/private/keyenv backup --output /absolute/private/encrypted-backup.json --verify-restore-home /absolute/private/restored-keyenv --yes
```

`--field` 只接受环境变量名称；不提供从参数或环境接收字段值的选项。`--purpose`、`--expires` 只用于非秘密元数据，未指定时保留已有值或采用空值；`--expires -` 清空到期日。`--yes` 只对密文 `backup`／`restore` 跳过已明确目标路径的再次确认，不适用于明文 `export`，也不跳过新设备初次恢复所需的主密码输入或拒绝覆盖。备份成功后验证失败会保留已创建的备份／恢复目录，应检查状态，不盲目重跑。

`status` 本地检查；`doctor` 检查当前 Drive 绑定，不测试凭据服务商 token。`list`/`inspect`/`run`/`export` 可加 `--offline` 显式读取已验证缓存；缓存为空或损坏不能回退使用。修改必须在线成功拉取；待上传版本不作为默认运行版本。正式 CLI 首次登录后自动使用本机加密令牌缓存；status 的 oauth_cache_state=available 只证明本机缓存绑定有效，不证明远端授权仍有效。实验 lab 保持 token 只留进程内。

本地 `vault.json` 是 0600 文件，包含已有格式的 wrapped-key header、加密的账号/应用/库绑定与密文 revision 状态。主密码不保存；独立 `unlock-cache.json`（0600，父目录 0700）保存未封装的数据密钥，绑定本库 header 摘要，跨命令及重启可用。该文件属于敏感解锁材料，能读取它即可解密本库；不要复制到其他设备、Drive、备份或 Git。密文备份仅包含 `vault.json` 内容。`lock` 删除此缓存，已有子进程中的值不会被收回。macOS/Linux 使用进程文件锁；子进程启动前释放锁。状态输出包含最近成功拉取时间、待上传数量及缓存使用标记，不代表全球即时一致。初始化会保存预生成 folder/header ID，失败重试复用它们，避免静默生成替代库。

Google 登录缓存是独立的 `oauth-login.json`（0600），使用本库数据密钥进行 AES-GCM 加密，绑定账号、客户端 ID、库 header 摘要及设备 ID；不进入 Drive、密文备份或 Git，也不复制到新设备。首次获取 refresh token 仍需本人授权一次，保持现有客户端和唯一 drive.file。刷新更新 access token，并原子保存新 token／轮换后的 refresh token；网络失败、授权撤销、权限或缓存错误不输出响应原文、不删除旧缓存、不静默扩权。令牌交换／刷新和 Drive 请求均拒绝重定向。新设备和恢复库单独本人授权，不借用源设备登录材料。

```sh
# 已有正式库：首次本人浏览器授权，主密码使用现有本机解锁缓存
./bin/keyenv --home /absolute/private/keyenv login
# 缓存令牌自动使用；这条可显式验证刷新，无需浏览器或主密码
./bin/keyenv --home /absolute/private/keyenv login --refresh
# 明确收到 AUTH_REQUIRED 后，本人重新授权；仍保留同客户端/同 scope
./bin/keyenv --home /absolute/private/keyenv login --reauthorize
# 只删除本机加密登录缓存，不撤销远端 grant
./bin/keyenv --home /absolute/private/keyenv logout
```

`lock` 删除解锁缓存、保留加密登录文件；缺少本机数据密钥时无法使用其中 token。损坏、错库／账号／客户端／设备、公开权限或符号链接缓存拒绝使用；损坏缓存如需重建，由用户明确 logout 后 login，不自动覆盖。正式库缓存已通过合成 OAuth/HTTP 测试和本机真实一次授权、跨进程复用、显式 Google 刷新、账号/库绑定校验及在线读取；缓存存在时的备份/恢复确认不复制登录材料。自然到期分支由合成测试覆盖，跨小时/跨周和真实撤权仍待实机验收。

已解锁的本机可用 `restore --input BACKUP --source-home EXISTING_HOME --yes` 恢复同一库的备份，无需再输入主密码；其他库的备份拒绝借用本机密钥。新设备不复制解锁缓存，初次 restore 输入一次主密码后建立自己的缓存。

更新输入保留未提及的字段；用途、到期日是脱敏元数据，不能填写秘密。到期日只提示，不代表服务商撤销。`resolve` 展示 head/entry/device ID 和字段名，用户明确选择一个版本；多父合并保留全部密文历史，同名不同 ID 也仅在显式确认时合并。加密备份恢复到新目录，原 ID/密文保持、设备 ID 更新；不会覆盖已有目录，未确认上传仍保持待处理。

完整交互模式中新条目字段数量默认 1（范围 1..64），更新已有字段默认 0（保留）；无效数字提示重填。单字段新条目的变量名默认 `API_KEY`，更新唯一已有字段时默认原名称，均可修改；多字段名称需明确输入。字段值（包括非秘密配置）仍隐藏输入，不默认真实秘密值。

## 直接 K=V 与登录终端自动加载

执行一次 `./bin/keyenv --home /absolute/private/keyenv shell-install`，为 `~/.zprofile` 安装仅含本机 CLI／库路径的加载代码。已有文件内容和权限保留，原文件另存到库目录下的 0600 私密备份；重复安装只更新标记块。没有把 CLI 复制到全局 PATH。自定义登录 profile 可用 `--profile /absolute/path`；使用 ZDOTDIR 的用户应明确选其 `.zprofile`。

打开新的登录 zsh 后可直接用 shell 函数：

```sh
keyenv set              # 逐行隐藏输入 K=V，空行保存；不把值放在命令后
keyenv keys --offline   # 只看字段名
keyenv sync             # 从 Drive 同步，并重新加载当前 shell
```

在 `set` 的隐藏提示内逐行填写，例如变量名 `OPENBB_TOEKN`、`CLOUDFLARE_API_TOKEN`、`DEEPSEEK_API_KEY`，每行输入变量名、等号和本人实际值；空行结束。值按第一个等号分割后字面保存，不加 shell 包裹引号，不进行变量或命令展开。新 key 各自作为一个加密版本条目保存；重填同 key 更新它，兼容更新原来服务分组里的同名字段，并保留其他字段。每次最多 64 个 key，受 v1 加密对象大小上限限制。同名字段出现在多个条目或存在冲突时拒绝自动选择。空行提交后，先把完整计划加密存入本机 vault.json 的绑定密文，再访问 Drive；录入等待不计入随后同步的 10 分钟超时，同步显示已确认条目数。网络失败返回 PENDING_ASSIGNMENTS，整批输入保留，直接 keyenv sync 续传，不需重新输入；AUTH_REQUIRED 时本人重新授权后再 sync。status/keys 同时报告 pending_assignments 和 pending_assignment_keys，草稿不用于环境注入，密文备份/恢复保留草稿。断电／丢响应时，先核对已上传 revision 的对象内容与父版本，再继续剩余批次，不创建重复版本。未完成草稿期间阻止新录入和其他条目写入；远端父版本变动拒绝覆盖。明确放弃时由本人 discard-set 确认，只清未完成草稿，保留已确认条目和待上传对象；随后再 sync，不把放弃当回滚。

值只存入本地 `vault.json` 和 Drive 密文。`.zprofile` 仅放加载代码，启动时 `env --offline --format zsh` 在内存中生成经严格引用的 export；不联网、不要求 Google 登录或主密码、不落明文文件，加载期间关闭并随后恢复 xtrace。系统环境／zsh 特殊变量不能作为全局 key。仅加载已确认的活跃条目；冲突、重复 key、损坏或锁定缓存报错；待上传版本不作为已确认版本。离线启动不代表云端最新，可明确执行 sync。

新登录 shell 和它的子进程，包括在此 shell 中运行工具的 agent，可直接读取这些环境变量。当前 shell 使用 `keyenv set/update/disable/resolve/sync` 成功后自动重新加载；也可 `keyenv_load` 手动重载。重载成功会清除上次由加载器注入、此次已移除的字段。已经运行的其他终端和 GUI 程序不会被追溯修改；它们需新建登录 shell 或重新加载。全局注入意味着这些子进程都能使用该密钥池；它不是进程隔离机制。lock 不会收回已经传出的环境值。

`OPENBB_TOEKN` 是用户指定拼写，目标程序必须读取相同字段名。本机 investment--mono 的 OpenBB 查询客户端已支持此名（兼容改动仍在相邻仓库工作区，未并入 personal-plugins），兼容旧 `OPENBB_ACCESS_TOKEN`，同时存在时优先新名。无需先创建 `openbb` 服务条目；早期预设脚本仍兼容。当前五个实际服务字段（OPENBB_TOEKN、CLOUDFLARE_API_TOKEN、DEEPSEEK_API_KEY、GROK_WEBHOOK_URL、GROK_WEBHOOK_AUTH_HEADER）已由本人在终端录入，返回 keys_saved、待上传为 0；独立新登录 shell 的子进程确认五者均存在且非空，只输出布尔值。未对服务商认证有效性或 webhook 执行作验收。

保留服务分组模式：`add/update` 可以维护多字段条目；`run --credential ENTRY -- EXECUTABLE ARG...` 在未启用全局加载的 shell 中按条目注入。可用 list/inspect 查看分组脱敏元数据。agent 不代填秘密，不直接调用 env 展示其输出，不读取缓存文件或打印整个环境。

`run` 使用参数数组启动程序，秘密作为字面环境变量注入；保持子进程输出，不能保证子进程不打印或发送秘密。工具状态写 stderr，只报告覆盖字段名。导出需用户交互确认，0600 原子创建且不覆盖；JSON 支持任意合法字符串。dotenv 使用单引号简易字面格式，对单引号、反斜线、美元符、控制字符等有解析歧义的值拒绝导出，应使用 JSON 或 `run`；不得 source 导出文件。

本机旧正式库一次 unlock 后，独立进程免主密码的 list/run、密文备份及恢复回读已实机通过。当前验收缺口：跨设备库接入、实际服务凭据录入、完整 update/run/export、转发信号和交互子进程体验、长期格式独立评审、安装/插件包装。已通过的正式库验收只录入公开测试值，不能因此宣布生产可用。不要向 agent 提供正式密码，也不要在旧 password-probe 窗口输入正式密码。

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

此实验入口保持每个进程重新输入测试密码，不创建正式 CLI 的解锁缓存。正式 CLI 的 setup／unlock 缓存模式见上文。主密码由你自行保管；缓存丢失后，Google 登录无法替代主密码解密备份。代码尽力清理持有的秘密字节，但 Go 运行时不保证所有内存副本被擦除。

测试完后手动删除该测试文件。此实验验证本地加密与手动解锁；下方 Drive 实验独立使用公开 fixture 密钥，不代表二者已经集成为正式凭据库。

## Google Drive 双客户端验证

### 一次配置

1. 使用你的个人 Google Cloud 项目，启用 Google Drive API。
2. 配置 Google Auth Platform 的 audience / consent，并创建 **Desktop app** OAuth client。
3. 下载 client JSON，保存在仓库之外。它不是 API key，也不应该粘贴到聊天或提交 Git。
4. 原型只申请 `https://www.googleapis.com/auth/drive.file`。测试期间若 audience 为 External/Testing，需要将自己的 Gmail 添加为 test user。

长期使用不能依赖 Testing 的令牌生命周期：带 Drive scope 的刷新令牌通常 7 天后过期。当前用户选择保留 Testing；若将来需要解除此限制，再独立完成 Production 所需配置并重新授权；生产状态、应用验证和令牌永久有效是不同概念。**本节 keyenv-lab 探针仅在进程内持有 token；正式 CLI 的加密缓存见上文。两者都不能凭本次模拟或即时刷新声称跨周登录已验证。**

### 第一台 Mac

```sh
./bin/keyenv-lab drive-probe --oauth-client /absolute/path/client.json
```

打开终端显示的 URL，在这台 Mac 的浏览器中授权。回调仅监听本机 127.0.0.1，使用随机 state 和 PKCE。

命令创建 `Keyenv Lab — test data only` 文件夹，在其中写入一份加密的固定虚构字符串，并重复上传同一个预生成 ID 以验证幂等行为。测试加密密钥是源码中公开的固定 fixture，**只用于虚构数据**。它不修改文件夹之外的文件，也不会自动删除测试文件。

授权前程序会明确提醒 fixture 密钥公开，成功 JSON 标注 `fixture_key_public: true` 和 `master_password_used: false`。没有导入数据或录入真实 token 的参数；不要修改固定 fixture 来存放真实凭据。此入口不会设置正式主密码或初始化正式 vault。指定已有文件夹时，必须具有原型专用标记、正确文件夹类型且未被删除，否则拒绝继续。

返回的 JSON 包含 `folder_id`、`created_file_id`、已读到的 fixture 数量与幂等结果。若后续操作失败，已创建的 folder ID 也会显示在 stderr，方便清理。

要以少量已有 fixture 验证真实分页，可在两次上传探针之后运行只读探针：

```sh
./bin/keyenv-lab drive-probe --oauth-client /absolute/path/client.json --folder FOLDER_ID --read-only --page-size 1
```

`--read-only` 必须提供明确已有实验文件夹，不创建文件夹或上传对象。`--page-size` 接受 1..100，默认仍为 100；列表保留最多 100 页及 2048 个对象的安全上限。输出 `list_pages`、`page_size` 和 `pagination_verified`；只有完整读取并校验全部已有 fixture 后才输出成功，实际读到多页时 `pagination_verified` 才为 true。这是读取时观察到的结果，不能证明全球即时一致或另一设备已同步。

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
