# 个人插件市场

两个市场索引指向相同的独立插件目录：Codex 使用 `.agents/plugins/marketplace.json`，Claude Code 使用 `.claude-plugin/marketplace.json`。每个插件有两份清单和一份技能，不共享身份或权限。`waypost` 的技能来自固定上游提交（见 `plugins/waypost/UPSTREAM.md`），`keyenv` 使用仓库现有技能；`codex-cu` 暂不加入市场，继续按其 README 单独注册。

本仓库当前使用公开 GitHub origin。市场名称或用户范围安装不提供保密性：需要私有分发时将仓库放在受控的私有 Git 仓库，并在每台设备配置 Git 读取权限。插件、市场和连接权限互不替代。不要将 token、私有配置、日志或本机状态加入仓库。

## 安装与验证

Git 市场的 Waypost 条目从同一仓库的分发提交 `e040011a7b0100e21a21fff2726af5dea8046de6` 拉取插件，其中包含 Linux x86-64、macOS ARM64 和 macOS Intel 的真实 bridge；本仓库的 `plugins/waypost` 源码目录本身不含二进制。该分发提交是在三平台构建及两宿主安装验证后创建的，条目固定完整 SHA，升级需审核并更新两份市场索引。这不会建立 Waypost 连接或启动接收。

```sh
codex plugin marketplace add miles1150/personal-plugins
codex plugin add waypost@personal-plugins
claude plugin marketplace add miles1150/personal-plugins
claude plugin install waypost@personal-plugins
```

以上默认 Git 市场指令适用于相关变更进入主分支后；当前 PR 可通过其分支测试。首次安装需 Git 网络访问。CI 也生成包含同一类三平台程序的 `personal-plugins-market` 本地工件，其市场条目引用包内目录，可用于离线安装。

从可信的本仓库成功 CI run 下载 `personal-plugins-market` 工件，核对 run 的仓库、提交及构建结果。解开 GitHub 工件外层 ZIP，在内容所在目录校验 `personal-plugins-market.tar.gz.sha256`（Linux 用 `sha256sum -c`；macOS 用 `shasum -a 256 -c`），再解开内部 tar，保留可执行权限。摘要只证明完整性，不证明发行者身份。将生成的 `personal-plugins-market/` 放在持久、受信任的本地路径；市场安装依赖该目录持续存在。

经 CI 构建并验证的 `distribution/waypost-v0.3.0` Git 分支也可直接作为完整市场来源（先确认该引用已存在，核验提交来源及 CI）。Codex 和 Claude 的分支参数不同：

```sh
codex plugin marketplace add miles1150/personal-plugins --ref distribution/waypost-v0.3.0
codex plugin add waypost@personal-plugins
claude plugin marketplace add 'miles1150/personal-plugins#distribution/waypost-v0.3.0'
claude plugin install waypost@personal-plugins
```

分发分支由通过三平台构建与双宿主安装测试的同一次工作流生成；创建时拒绝覆盖同名引用。Git 分支仍可能由仓库拥有者修改，不能将名称视为密码学签名。检查实际分支提交、生成工件和仓库权限，升级时采用新的版本引用；已有同名市场应先核对原来源，再按宿主命令移除或更新。

在该市场目录运行：

```sh
codex plugin marketplace add "$PWD"
codex plugin add waypost@personal-plugins
# 可选，独立安装：codex plugin add keyenv@personal-plugins
codex plugin list --marketplace personal-plugins --json
```

Claude Code 使用：

```sh
claude plugin validate "$PWD"
claude plugin validate "$PWD/plugins/waypost"
claude plugin marketplace add "$PWD"
claude plugin install waypost@personal-plugins
# 可选：claude plugin install keyenv@personal-plugins
claude plugin list
```

启动新会话后检查技能可见（Claude 中 `/waypost:waypost`；Codex 中使用技能选择器）。自动选择由模型决定，需用实际请求测试。Codex IDE 扩展的插件支持与 MCP 配置是不同能力；请核对所用版本的官方文档。上面的本地路径市场依赖该目录的存在；不要在移动或删除目录后假定市场仍可用。

## Waypost 连接和 CLI

当前已发布的 0.3.0 市场包没有 MCP 配置。源码中的 0.4.0 准备了远程 MCP 连接，需经独立构建、验证和市场 pin 更新后才会随市场安装。认证由宿主通过服务发现和动态注册完成；用户需在浏览器明确登录并授权。先调用 `waypost_describe` 确认 `space_membership`、身份和能力。不要把凭据放在插件清单、命令行、聊天或日志。连接具有空间内共享资料读写能力，不是只读 token。

新机自动注册与独立 bridge 身份的验收边界见 [Waypost 接入准备](waypost-native-onboarding.md)。

可选 `waypost-bridge` 负责通知，不是 Waypost 服务本身；不需要 bridge 也可使用 MCP。源码市场的 Waypost 插件不含二进制；下述构建可生成包含当前平台 bridge 的插件目录。`skills/waypost/scripts/bridge.py doctor` 使用 Python 3 检查包内文件、平台、固定来源标识、插件版本和 SHA-256；`bridge` 后接明确的 bridge 参数才会运行。脚本不会回退到 PATH，也不会自动下载、安装或启动；用户显式传入 `run` 会启动 bridge。宿主缓存路径以实际技能位置为准，例如 `python3 /ABS/PATH/TO/skills/waypost/scripts/bridge.py doctor`。插件不提供 Python。

要构建随包 CLI，在对应平台预先准备干净的 [Waypost 仓库](https://github.com/miles1150/waypost) 固定提交 `9f70cbf3779211544739ff7998b93895b8618944`、Python 3.11.4+、Rust 1.91+、Cargo、编译工具和已缓存的锁定依赖，审查源码及构建环境。没有 Cargo 或依赖缓存时构建会失败；插件不会代为安装。下面命令只使用已有源码和依赖，`cargo build --locked --offline` 失败时不会下载；输出目录必须不存在，且位于 Waypost 源码树之外：

```sh
python3 tools/waypost/build_bundle.py --source /ABS/CLEAN/WAYPOST --output /ABS/NEW/waypost
```

生成目录是完整的单平台插件，而非系统安装。构建器检查固定提交和干净工作区，然后从 Git 对象导出该提交进行构建，忽略文件不会进入源码快照；它还检查双清单版本、构建产物 ELF/Mach-O 的操作系统及 CPU，并记录摘要和工具链版本。复制生成的 `waypost` 目录到经审查的市场副本 `plugins/waypost`，再按上面的市场流程安装并运行缓存中脚本的 `doctor`；勿将用户数据放进插件目录。隔离测试中 Codex CLI 0.159.2 能复制测试 ELF、保留执行权限并显式运行它；这不是实际 bridge 的验收。每种平台需在对应机器构建各自的包。允许 Linux x86-64/ARM64 和 macOS x86-64/ARM64；当前仅用 Linux x86-64 假编译器及本机测试程序验证了流程，没有产出真实 bridge 或验证其他平台。Linux glibc、macOS 部署目标、签名和其他宿主复制权限还需在目标机器验收。

`--locked` 和源码固定只能约束输入的一部分：编译器、系统库、Cargo 配置与依赖缓存仍影响产物，不能称为位对位可复现。摘要随包生成，只检查包内一致性；攻击者同时替换文件和摘要仍可通过。只有从本人审核的源码和可信构建环境产出、并通过可信渠道校验整个插件包，才能建立来源信任。参考 [Waypost receiver setup](https://github.com/miles1150/waypost/blob/9f70cbf3779211544739ff7998b93895b8618944/docs/receiver-setup.md) 配置真实接收；不假设已授权或已有后台服务。

插件缓存及构建目录必须由可信用户控制。启动器在支持描述符执行的系统上对打开的文件计算摘要并通过同一描述符执行，可防止校验后替换路径，但不能阻止对同一文件的原地修改；不支持描述符执行的系统使用路径，存在校验与执行之间的替换窗口。它不是抵御同账号恶意进程的沙箱，也没有包签名。更新或回滚时使用完整且来源可信的匹配版本包，重新安装后检查 `doctor`；不要只替换二进制或修改版本号。已有后台服务可能仍运行旧二进制，先停止并备份状态，再单独更新和验收。

bridge 的配置、token 和持久状态保存在插件缓存之外。后台接收需额外的服务或调度和用户授权；通知、读取、ACK、处理是不同阶段。升级或卸载前停止对应 bridge，备份并保留状态目录和私密配置，以免重复通知；卸载插件不会撤销连接或移除 CLI/后台服务。独立停止及移除服务、程序、连接凭据的具体步骤按上游指南执行，不删除状态作为故障恢复方法。

Keyenv CLI 的安装、升级和个人库接入分别见 [安装](keyenv-installation.md)、[个人接入](keyenv-local-handoff.md)；插件卸载不删除密钥库或撤销 Drive 授权。技能不捆绑 CLI、文档或 OAuth 密文，使用时按技能指示读取匹配版本文档。

## 更新和卸载

本地路径市场：在仓库检查并更新到经过审查的提交，修改的插件同时递增两份清单版本；对 Codex 再运行 `codex plugin add waypost@personal-plugins`（或相应插件名），启动新会话并用 `codex plugin list --marketplace personal-plugins --json` 检查实际版本及技能。隔离环境的 Codex CLI 0.159.2 已验证重新 add 可从 0.1.0 更新到 0.1.1。Claude 本地市场可直接读取文件，托管市场使用缓存，二者更新方式不同；按该版本的插件更新命令操作并验证。Git 市场可用 Codex 的 `codex plugin marketplace upgrade personal-plugins` 刷新；若 ref 固定，应先按受控流程改到目标 ref。更新索引不等于升级独立 CLI 或连接。

```sh
codex plugin remove waypost@personal-plugins
# 当不再使用任何本市场插件时：
codex plugin marketplace remove personal-plugins
claude plugin uninstall waypost@personal-plugins
claude plugin marketplace remove personal-plugins
```

先分别卸载不再需要的插件，再移除市场；检查宿主实际列表。回滚至已审查的旧提交和旧插件版本，重装并验证。不要通过删除缓存假定外部连接、权限或后台服务已撤销。

## 验证范围

本仓库的 `python3 -m unittest discover -s tests -p 'test_plugin*.py'` 检查市场引用、双清单版本、缺失和损坏的产物、平台及参数转发，并用假 Cargo 和本机测试 ELF 验证打包流程；不证明真实 Rust 构建。插件 CI 在 Linux、macOS ARM 和 Intel 上运行测试和真实桥接程序的包构建及无配置启动检查，但 Linux 专用 ELF 测试在 macOS 跳过；未安装 Codex 时 OAuth 宿主模拟测试也会跳过，不代表宿主验收。固定上游的 bridge 没有 `--help` 命令，该无配置检查只验证它会输出用法并退出，不连接服务。CI 生成的工件是待验收构建，不自动发布。下载后先校验 `.tar.gz.sha256`，再解开内部 tar 包以保留可执行权限；外层 GitHub artifact ZIP 不保证保留权限。Claude Code、Waypost 连接、后台唤醒和双设备必须分别验收，不能由静态测试推断。官方规范：[Codex 打包](https://developers.openai.com/plugins/build/plugins)、[Claude 市场](https://code.claude.com/docs/en/plugin-marketplaces)、[Claude 插件组件](https://code.claude.com/docs/en/plugins/components)。
