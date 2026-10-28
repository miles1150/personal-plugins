# 个人插件市场

两个市场索引指向相同的独立插件目录：Codex 使用 `.agents/plugins/marketplace.json`，Claude Code 使用 `.claude-plugin/marketplace.json`。每个插件有两份清单和一份技能，不共享身份或权限。`waypost` 的技能来自固定上游提交（见 `plugins/waypost/UPSTREAM.md`），`keyenv` 使用仓库现有技能；`codex-cu` 暂不加入市场，继续按其 README 单独注册。

本仓库当前使用公开 GitHub origin。市场名称或用户范围安装不提供保密性：需要私有分发时将仓库放在受控的私有 Git 仓库，并在每台设备配置 Git 读取权限。插件、市场和连接权限互不替代。不要将 token、私有配置、日志或本机状态加入仓库。

## 安装与验证

先在可信位置检出本仓库、审查版本，然后在仓库根目录运行。下面操作只添加本市场，不会建立 Waypost 连接或安装 Keyenv/bridge。市场与插件代码来自 Git，确认来源和变更后再启用。

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

启动新会话后检查技能可见（Claude 中 `/waypost:waypost`；Codex 中使用技能选择器）。自动选择由模型决定，需用实际请求测试。Codex IDE 扩展的插件支持与 MCP 配置是不同能力；请核对所用版本的官方文档。上面的本地路径市场依赖该检出的存在；不要在移动或删除目录后假定市场仍可用。

## Waypost 连接和 CLI

技能不会创建 MCP 连接。按 [Waypost 使用指南](https://github.com/miles1150/waypost/blob/9f70cbf3779211544739ff7998b93895b8618944/docs/user-guide.md) 和宿主的私密设置接入已有服务，凭据只放在宿主支持的安全位置。先调用 `waypost_describe` 确认 `space_membership`、身份和能力。当前上游文档使用独立连接凭据，不在本插件中假设 OAuth 已提供。不要把凭据放在插件清单、命令行、聊天或日志；不能安全配置时停止。连接具有空间内共享资料读写能力，不是只读 token。

可选 `waypost-bridge` 是独立程序，负责通知，不是 Waypost 服务本身；不需要 bridge 也可使用 MCP。插件内 `skills/waypost/scripts/bridge.py` 支持 `doctor` 和显式 `bridge` 子命令，使用 Python 3，寻找 PATH 中的 `waypost-bridge`；它不下载或启动服务。可通过技能文件路径定位脚本，例如 `python3 /ABS/PATH/TO/skills/waypost/scripts/bridge.py doctor`；宿主缓存路径以实际技能位置为准，不要依赖系统 PATH 自动包含插件目录。Claude Code 支持插件根部 `bin/`，但本包不依赖这个宿主专属行为。

用户选择需要后台接收时，按 [Waypost receiver setup](https://github.com/miles1150/waypost/blob/9f70cbf3779211544739ff7998b93895b8618944/docs/receiver-setup.md) 选择核验过的上游构建或固定提交源码。上游记录的构建包括 Linux x86-64、macOS x86-64/ARM64；Linux ARM64 需自行构建，其他平台未验证。校验下载摘要和提交来源，检查 `waypost-bridge --help`，将二进制安装到独立位置并确保 PATH 可见。插件安装不提供 Python 或该二进制。Windows 和具体宿主缓存中的可执行行为未验收。

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

本仓库的 `python3 -m unittest discover -s tests -p 'test_plugin*.py'` 检查市场引用、双清单版本及 bridge 包装器的缺失/参数转发。Codex 隔离安装与技能枚举需针对本次提交实际运行；Claude Code、macOS、Windows、真实 Waypost 连接、后台唤醒和双设备必须分别验收，不能由静态测试推断。官方规范：[Codex 打包](https://developers.openai.com/plugins/build/plugins)、[Claude 市场](https://code.claude.com/docs/en/plugin-marketplaces)、[Claude 插件组件](https://code.claude.com/docs/en/plugins/components)。
