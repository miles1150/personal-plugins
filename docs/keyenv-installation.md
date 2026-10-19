# Keyenv 通用安装

适用于使用自己 Google 账号、OAuth 客户端和密钥库的用户。维护者的新电脑接入自己的现有库，请改走 [个人接入指南](keyenv-local-handoff.md)。两条路径共用同一个 CLI；只有个人路径使用仓库附带的加密客户端包。

## 1. 安装 CLI

截至 2026-10-19，已发布版本为实验性 [keyenv-v0.1.0-rc.1](https://github.com/miles1150/personal-plugins/releases/tag/keyenv-v0.1.0-rc.1)。从该页面下载对应平台的压缩包和同一版本的 `SHA256SUMS`，放入同一个下载目录。通用安装只需一个平台包，不需要 `personal-connect` 包。

| 系统 / CPU | 压缩包后缀 |
| --- | --- |
| macOS Apple Silicon | `darwin-arm64.tar.gz` |
| macOS Intel | `darwin-amd64.tar.gz` |
| Linux ARM64 | `linux-arm64.tar.gz` |
| Linux x86_64 | `linux-amd64.tar.gz` |

预编译运行无需 clone、Go、Xcode、root 或钥匙串。需要系统的 `tar`，以及 macOS `shasum` 或 Linux `sha256sum`。Windows 暂无分发包；自动加载功能仅支持登录 zsh。

下面以 macOS Apple Silicon 为例，在下载目录执行。其他平台只改 `keyenv_platform`。整个代码块在子 shell 中执行，任何一步失败即停止；已解压目录不会被覆盖。

```sh
(
  set -eu
  keyenv_version=0.1.0-rc.1
  keyenv_platform=darwin-arm64
  keyenv_package="keyenv-$keyenv_version-$keyenv_platform"
  keyenv_archive="$keyenv_package.tar.gz"
  # 总清单包含所有平台；只校验自己下载的这个包。
  awk -v name="$keyenv_archive" '$2 == name {print; count++} END {if (count != 1) exit 1}' \
    SHA256SUMS > "$keyenv_archive.sha256"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$keyenv_archive.sha256"
  else
    shasum -a 256 -c "$keyenv_archive.sha256"
  fi
  test ! -e "$keyenv_package"
  tar -xzf "$keyenv_archive"
  mkdir -p "$HOME/.local/bin"
  /bin/sh "$PWD/$keyenv_package/install-local.sh" \
    "$PWD/$keyenv_package" "$HOME/.local/bin"
  "$HOME/.local/bin/keyenv" version
)
```

应看到校验 `OK` 和版本 `0.1.0-rc.1`。校验和检测损坏，不是发布者签名；从可信发布页面取得包和清单，不执行未校验的下载脚本。macOS 包尚未签名或公证，遇系统安全提示先核实来源，遵循系统正常流程，不关闭安全机制。

安装器会再次检查包内文件校验和。相同 binary 可重复安装；不同旧版本默认拒绝，确认升级后在安装器最后追加 `--replace`，保留稳定的 `$HOME/.local/bin/keyenv` 路径。目标 bin 目录必须已存在，拒绝符号链接目标。安装器不改 PATH 或 shell 配置，接下来先使用完整路径。

## 2. 准备自己的 Google 客户端

Google OAuth 应用表示“哪个客户端请求访问你的 Drive”，不是另一个云盘账号。数据仍存放在你的个人 Drive。客户端配置识别应用，浏览器授权确定账号与访问权限，主密码负责解密；三者不能互相替代。

在自己的 Google Cloud 项目启用 Drive API，配置 Google Auth Platform，并创建 **Desktop app** 客户端。只配置 `https://www.googleapis.com/auth/drive.file`；如使用 External / Testing，将自己的账号加入测试用户。本人完成政策确认和浏览器授权。不要借用维护者的个人客户端包，也不要为了发现文件扩大为整盘权限。

保存自己的完整客户端 JSON 到仓库外，例如 `$HOME/.config/keyenv/oauth/client.json`。父目录应为 0700，文件为 0600；不要粘贴内容到聊天、日志或 Git。已有客户端优先使用已保存的 JSON，不假定旧 secret 可以重新下载，不必重建客户端。

准备私密父目录（不预建 `vault` 目录）：

```sh
mkdir -p "$HOME/.config/keyenv/oauth"
chmod 700 "$HOME/.config/keyenv" "$HOME/.config/keyenv/oauth"
# 将自己保存的客户端 JSON 安全放到上述目录后，再执行：
chmod 600 "$HOME/.config/keyenv/oauth/client.json"
```

## 3. 首台设备创建自己的库

在本人控制的交互终端运行，将示例邮箱换成自己的邮箱：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" setup \
  --oauth-client "$HOME/.config/keyenv/oauth/client.json" \
  --account YOUR_GOOGLE_ACCOUNT
```

按中文提示确认，隐藏输入一次自己保管的主密码，在本机浏览器完成授权。没有 `--folder` 表示创建新 Drive 库，会另行要求创建确认。成功后安全记录输出的 `folder_id`，供新设备接入；它不是主密码或访问令牌。首次无需先运行 `init`，以后维护无需重做 setup。

setup 保存本机解锁密钥及加密的 Google 登录缓存，不保存主密码。后续维护无需重复输入主密码，正常联网会自动刷新 Google token；缓存失效时按 [故障处理](keyenv-usage.md#同步与故障处理) 本人重新授权。Google 登录不能恢复遗失的主密码。

## 4. 其他设备接入自己的已有库

先安装同一 CLI，安全放好同一 OAuth 客户端 JSON，再运行：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" setup \
  --oauth-client "$HOME/.config/keyenv/oauth/client.json" \
  --account YOUR_GOOGLE_ACCOUNT --folder YOUR_EXISTING_FOLDER_ID
```

`YOUR_EXISTING_FOLDER_ID` 是原库 URL 中 `/folders/` 后的 ID，可从原设备 `status` 的 `folder_id` 获得。必须明确指定；目前不会按文件夹名自动搜索。新电脑本人使用同一主密码解锁，在浏览器独立授权。不要复制其他设备的 `unlock-cache.json` 或 `oauth-login.json`。

目标库目录必须不存在。已接入的本机库使用 `sync`，不再执行 setup；目录存在但不完整时保留文件先核查，不删除重建或另建云盘库。

## 5. 开始使用

接着阅读 [日常使用](keyenv-usage.md)：用 `set` 隐藏录入 K=V，选择是否安装 `.zprofile` 自动加载。安装 CLI 不会自动修改 shell，也不自动注入任何变量。

源码构建、客户端打包和实验探针见 [开发说明](keyenv-development.md)。当前验收范围见 [验证状态](keyenv-validation.md)。
