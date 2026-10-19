# Keyenv 个人新电脑接入

本页只适用于本仓库维护者接入自己的已有库，不是通用安装配置。其他用户请看 [通用安装](keyenv-installation.md)，日常操作见 [使用指南](keyenv-usage.md)。更新日期：2026-10-19。

## 现有配置：继续复用，不重新创建

| 项目 | 已确定值 |
| --- | --- |
| Google 账号 | `rentlifelee@gmail.com` |
| Google Cloud 项目 | `keyenv-personal` |
| Desktop 客户端 | `Keyenv Local Desktop`（授权品牌 Keyenv Personal） |
| 唯一权限 | `https://www.googleapis.com/auth/drive.file` |
| 正式 Drive folder ID | `19F-aCbnpf_fKsKQhgc6wCDOsshoVFUL7` |
| 正式 vault ID | `5d2438266716c40c4df12649cf2e191e` |
| 推荐本机库 | `$HOME/.config/keyenv/vault` |
| 推荐 CLI 路径 | `$HOME/.local/bin/keyenv` |

这些标识不是访问令牌，也不能代替 Google 授权或主密码。要求 folder ID 是为了明确接入原库，避免误建空库；当前不会按名称自动发现。此处已给出，不需新电脑重复询问。

继续保留现有客户端、旧 secret、Testing 状态和 drive.file，不重建客户端、不部署 Google 发布页面、不推进 Production。完整客户端 JSON 不再要求从旧电脑手工搬运：当前已授权通过 Git / 个人包分发公开配置及加密 secret，新电脑用原主密码解密。两个文件配对并绑定原库 header，不能单独更改公开配置。

## 1. 安装通用 CLI，再取得个人包

先按 [通用安装第 1 节](keyenv-installation.md#1-安装-cli) 安装对应 CPU 的 CLI。无需 clone、Go、Xcode 或钥匙串，也不复制旧电脑的缓存。

从同一 [0.1.0-rc.1 发布页面](https://github.com/miles1150/personal-plugins/releases/tag/keyenv-v0.1.0-rc.1) 再下载 `keyenv-0.1.0-rc.1-personal-connect.tar.gz`；与该版本的总 `SHA256SUMS` 放在同一下载目录。校验后解压（已解压目录不覆盖）：

```sh
(
  set -eu
  keyenv_package=keyenv-0.1.0-rc.1-personal-connect
  keyenv_archive="$keyenv_package.tar.gz"
  awk -v name="$keyenv_archive" '$2 == name {print; count++} END {if (count != 1) exit 1}' \
    SHA256SUMS > "$keyenv_archive.sha256"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$keyenv_archive.sha256"
  else
    shasum -a 256 -c "$keyenv_archive.sha256"
  fi
  test ! -e "$keyenv_package"
  tar -xzf "$keyenv_archive"
)
```

个人包只含接入／校验脚本、VERSION、内部清单及 `oauth/client.public.json` / `oauth/client.secret.enc.json`，没有 CLI 或任何服务字段值。其他用户无需下载它。

## 2. 本人在终端运行接入助手

仍在上述下载目录，由本人执行：

```sh
/bin/sh "$PWD/keyenv-0.1.0-rc.1-personal-connect/connect-existing.sh" \
  rentlifelee@gmail.com 19F-aCbnpf_fKsKQhgc6wCDOsshoVFUL7
```

新设备按中文提示确认、隐藏输入一次**原库主密码**，然后亲自在本机浏览器完成同一账号的授权。不要把主密码、验证码或回调 URL 发给 agent。接入助手会：

1. 校验个人包，使用已安装的稳定 CLI。
2. 将客户端解密到仓库外 `$HOME/.config/keyenv/oauth/client.json`，使用私密目录／0600 文件；原有相同配置复用，不同配置拒绝覆盖。
3. 用同一次主密码输入核对原 Drive header 并接入已有库；独立建立本机解锁与加密登录缓存，不保存主密码。
4. 成功后为 `${ZDOTDIR:-$HOME}/.zprofile` 安装只含路径的加载块，并只列字段名。

若已有本机 `vault.json`，助手只执行 `sync`，不重做 setup。目录存在但缺少 `vault.json` 会停止并保留文件；不要删除重试。`LOCAL_UNLOCK_REQUIRED` 时由本人对该 home 执行一次 `unlock`；`AUTH_REQUIRED` 时本人 `login --reauthorize`，再重跑助手。不要复制其他设备的 `unlock-cache.json`、`oauth-login.json` 或借用登录状态。

脚本可用第三个参数指定仓库外绝对库路径；非默认 CLI 用 `KEYENV_CLI=/absolute/bin/keyenv`。它只修改登录 zsh，不会让已有其他终端或 GUI 程序的环境即时变化。

## 3. 新登录 shell 验证：只看状态和字段名

打开新的登录 zsh：

```sh
keyenv version
keyenv status
keyenv keys --offline
```

应是上表的 folder/vault，`pending_uploads=0`、`pending_assignments=0`，并包含下列五个字段。若要检测进程注入，按 [使用指南](keyenv-usage.md#程序和-agent-怎么使用) 只输出存在／非空布尔值，不打印字段值。agent 明确通过 `/bin/zsh -lc` 运行目标程序。

| 已有字段 | 用途 / 消费约定 |
| --- | --- |
| `OPENBB_TOEKN` | OpenBB 消费者读取的变量名；保留本人指定拼写，不自动改成 TOKEN 或旧名字 |
| `CLOUDFLARE_API_TOKEN` | Cloudflare API token；具体权限以服务端 token 配置为准 |
| `DEEPSEEK_API_KEY` | DeepSeek API 调用 |
| `GROK_WEBHOOK_URL` | Grok Bot webhook 地址，本身也按敏感配置处理 |
| `GROK_WEBHOOK_AUTH_HEADER` | 完整 HTTP 鉴权头，消费者不要重复添加头名称或 Bearer 前缀 |

不需要重新录入这五个值，先从现有库同步。本机早期记录已验证五字段录入、上传完成及登录 shell 注入；这不证明新电脑已同步，也不证明服务商密钥有效。OpenBB 消费者需实际读取 `OPENBB_TOEKN`；相邻投资仓库的兼容改动曾在其本地工作区，不能据本仓库文档认定已经合并或在 Air 生效。

## 使用源码 checkout 的替代入口

只有开发或需要尚未发布的改动时才 clone。先阅读适用的 AGENTS.md 和本仓库文档；检查 `git status`，保留未提交修改，`git fetch origin main` 后仅在安全时 `git merge --ff-only origin/main`，分叉或冲突停止核查，不 reset、不覆盖、不 force push。

在仓库根目录，已安装 CLI 时显式指定源码中的 bundle：

```sh
KEYENV_OAUTH_BUNDLE="$PWD/tools/keyenv/config/oauth" \
  /bin/sh tools/keyenv/scripts/connect-existing.sh \
  rentlifelee@gmail.com 19F-aCbnpf_fKsKQhgc6wCDOsshoVFUL7
```

如果明确选择源码构建，需 Go 1.26+，在仓库根目录改用：

```sh
KEYENV_BUILD_FROM_SOURCE=1 /bin/sh tools/keyenv/scripts/connect-existing.sh \
  rentlifelee@gmail.com 19F-aCbnpf_fKsKQhgc6wCDOsshoVFUL7
```

源码模式才会调用 Go，使用源码树的 `tools/keyenv/bin/keyenv`。启用 shell 加载后须保留此稳定 checkout 路径；正常预编译接入不走这条路径。

## 给新电脑 agent 的接续 prompt

```text
接续 miles1150/personal-plugins 的 Keyenv 个人接入。先读仓库最新 README、docs/keyenv-local-handoff.md 和 docs/keyenv-usage.md，按最新交接继续，不重新需求访谈。优先安装已发布的预编译 CLI 和配套 personal-connect 包，先校验，不要求 Go、Xcode 或钥匙串。若已有 checkout，保留未提交修改，安全 fetch/快进 main；不覆盖或强制更新。
复用文档中的现有 Google 账号、Keyenv Local Desktop、drive.file 和正式 folder ID，使用已授权的公开配置＋加密 secret 包，不重建客户端、不新建云盘库、不手工复制旧机缓存。本人在终端隐藏输入一次原主密码并亲自完成浏览器授权，秘密不进聊天、参数、日志或 Git。
已有本机库仅 sync。完成后只检查库绑定、待处理数量、五字段名及登录 zsh 子进程中的存在/非空布尔值，不打印值。核对 OpenBB 消费者实际使用 OPENBB_TOEKN。没有单独任务授权不调用服务商 API、发送 webhook 或部署。准确区分本机接入、双机同步、恢复和服务商认证的已验证/待完成状态。
```

当前证据见 [验证状态](keyenv-validation.md)。双机互读与恢复仍需本人在两台机器完成，不把脚本已准备或包已发布算成验收。旧一次性验收脚本、下载客户端和发布网站的过程见 [历史交接](archive/keyenv-local-handoff-2026-10-13.md)，不再作为操作入口。
