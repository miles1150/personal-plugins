# Keyenv 日常使用

先完成 [通用安装与 setup](keyenv-installation.md)，或维护者的 [个人新电脑接入](keyenv-local-handoff.md)。setup 只管设备接入；下面的密钥维护不重做 setup、不创建新的 Drive 库。

## 录入和修改 K=V

尚未启用 shell 加载时，使用固定 binary 和库路径：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" set
```

在**隐藏提示内**逐行输入 `变量名=实际值`，最后提交一个空行保存。不要把赋值写在命令参数中，不要先执行 shell 的 `export`，也不要发给 agent。输入时无回显是正常行为；出现下一次 K=V 提示说明程序仍在等待下一行，空行后才开始保存和同步。

例如 `EXAMPLE_API_KEY=...` 仅表示输入格式，实际值由本人填写。按第一个 `=` 分割，后面的内容原样保存；不执行变量展开或命令替换。不要额外加 shell 包裹引号，引号也会成为值。支持空值，每批最多 64 个字段，并受对象大小上限限制。

再次 `set` 同一字段名即更新它，未提及字段保留。无需先创建服务条目或回答字段数量问题。新字段各自建立加密条目，兼容已有服务分组中的同名字段；重复字段或版本冲突会拒绝自动选择。

setup 后使用本机解锁缓存，维护不再输入主密码；新增或更换秘密值仍由本人隐藏输入。整个录入计划在联网写入前加密保存，网络失败的续传方式见下文。

## 选择启用 .zprofile 自动加载

希望登录终端及其子进程直接获得全部活跃字段时，执行一次：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" shell-install \
  --profile "${ZDOTDIR:-$HOME}/.zprofile"
```

这会修改所选 profile：保留原内容与权限、留存私密备份，新增只含本机 CLI／库路径的受管块。不会把密钥值写入 `.zprofile` 或明文 `.env`。应使用稳定安装路径，避免引用随后被删除的下载目录。已有同名环境变量会被库中值覆盖。

打开新的登录 zsh 后可直接执行：

```sh
keyenv set
keyenv keys --offline
keyenv sync
```

启动时只读已确认的本机密文缓存：不联网、不打开 Google、不重复输入主密码。离线缓存不代表云端最新，需要时主动 `sync`。待上传版本与录入草稿不作为已确认字段注入。

shell 函数形式的 `keyenv set/update/disable/resolve/sync` 成功后会重新加载当前 shell，也可用 `keyenv_load` 手动重载。已经运行的其他终端和 GUI 程序不会自动更新，需新开登录 shell 或在已安装加载器的 shell 中重载。`lock` 也不能收回已经传给进程的变量。

全局加载后，该 shell 的所有子进程都可以访问这些值。仅希望单个程序取得指定条目时，使用下方 `run` 模式，不必安装全局加载。

## 程序和 agent 怎么使用

程序从自己的进程环境读取约定字段名；环境变量存在不代表服务商认证有效。agent 的执行器未必加载 `.zprofile`，可以明确在登录 zsh 内运行目标程序，例如：

```sh
/bin/zsh -lc 'python3 app.py'
```

在目标项目目录运行上述命令。不要将变量值拼入命令行；应用内部读取环境。仅核对注入是否成功时，可以输出布尔值：

```sh
/bin/zsh -lc 'python3 -' <<'PYTHON'
import os
name = "EXAMPLE_API_KEY"
print({"present": name in os.environ, "nonempty": bool(os.environ.get(name))})
PYTHON
```

将示例字段名改成要检查的名称。不要执行 `printenv`、环境全量打印或直接显示 `keyenv env` 的结果。使用凭据访问外部服务、部署或发送 webhook，仍须有对应任务授权；字段存在本身不授予这些操作权限。

保留按服务分组的方式（以下通过完整路径调用，不依赖全局加载）：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" add \
  --field EXAMPLE_API_KEY --purpose 示例服务 example
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" update \
  --field EXAMPLE_API_KEY example
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" list --offline
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" inspect --offline example
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" run \
  --offline --credential example -- python3 app.py
```

条目名 `example` 和变量名 `EXAMPLE_API_KEY` 是两件事；`run` 选择条目。`add/update --field` 跳过非必要字段问题，值仍隐藏输入。用途和到期日是非秘密元数据。`run` 按参数数组启动程序，不进行 shell 求值；程序自身仍可能打印或外发值，应选择与任务有关的程序。

## 同步与故障处理

下面的短命令假定已启用上述 shell 函数；未启用时使用 `"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault"` 替代 `keyenv`。

```sh
keyenv status
keyenv keys --offline
keyenv sync
```

`status` 只检查本地状态。`oauth_cache_state=available` 只说明本机缓存可用，不证明远端授权仍有效；`doctor` 可检查 Drive 绑定，不验证 OpenBB 等服务商密钥。

| 现象 / 错误 | 处理 |
| --- | --- |
| 隐藏 K=V 提示反复出现 | 每行回车后继续等待；最后再输入一个空行才提交 |
| `PENDING_ASSIGNMENTS` | 整批输入已加密保存，直接 `sync` 续传，不重新输入或重做 setup |
| `PENDING_UPLOAD` / `REMOTE_UNAVAILABLE` | 保留本机文件，检查 `status`、网络和授权，恢复后 `sync`；不要仅据错误码认定输入已丢失 |
| `AUTH_REQUIRED` | 本人在终端运行 `keyenv login --reauthorize`，浏览器完成原账号、原客户端、仅 drive.file 的授权，然后 `sync` |
| `LOCAL_UNLOCK_REQUIRED` | 本人运行 `keyenv unlock`，隐藏输入原主密码一次；不要通过参数、环境变量或聊天传递 |
| `CONFLICT` | 用 `list/inspect` 看脱敏元数据，再由本人明确 `resolve ENTRY`；不自动选胜者 |
| `DECRYPTION_FAILED` / 损坏或不安全文件 | 停止覆盖与重建，保留库和备份，核查密码、绑定和权限；不要盲目反复 setup |
| `INTERACTION_REQUIRED` | 需要本人交互终端操作，不用管道传密码，不由 agent 模拟隐藏输入 |

未完成录入草稿会阻止新的录入和其他条目写入；`status/keys` 报告草稿数量与字段名。明确放弃时由本人运行 `discard-set` 并确认；它不是回滚，不删除已确认上传的版本。父版本变化必须显式处理，不能强行覆盖。

Google 登录缓存将 access/refresh token 加密保存为本机 `oauth-login.json`，正常联网自动刷新。可以用 `login --refresh` 主动验证刷新；缓存被撤销或过期仍可能需要本人重新授权，不保证永久免登录。`logout` 只删除本机登录缓存，不撤销 Google grant；不要为了排障擅自删除缓存、扩权或重建客户端。

## 备份与恢复

密文备份和离线恢复不需要 Google 登录。下例的备份文件、恢复目录必须是新的仓库外绝对路径；已有文件拒绝覆盖：

```sh
"$HOME/.local/bin/keyenv" --home "$HOME/.config/keyenv/vault" backup \
  --output /absolute/private/NEW-backup.json \
  --verify-restore-home /absolute/private/NEW-restored-vault --yes
```

已解锁的本机不再输入主密码；完成后应返回 `backup_and_restore_verified`、`recovery_verified=true`。该验证比较密文及解密后的版本、待上传状态和绑定，确认恢复的设备 ID 已改变；不是双机 Drive 验收。备份不包含本机解锁材料、OAuth token 或完整客户端 JSON。

新电脑可以从已有密文备份恢复到新目录：

```sh
"$HOME/.local/bin/keyenv" --home /absolute/private/NEW-restored-vault restore \
  --input /absolute/private/encrypted-backup.json
```

本人隐藏输入原主密码一次，建立此设备自己的解锁缓存。若是已解锁的同一台电脑，可额外指定 `--source-home EXISTING_HOME --yes` 复用同库解锁材料；其他库不能借用。

恢复会保留备份里的客户端绝对路径，当前没有跨用户名的路径重绑定命令。原路径仍可用时，安全放好对应客户端配置后本人 `login`，不复制登录缓存、不重做 setup。原路径在新电脑不可用时，先保留离线恢复结果；日常联网接入另按安装指南连接现有 Drive 库，使用新的本机目录，不覆盖恢复目录或手工改密文。跨设备恢复后联网仍待验收。

## 本机文件与生命周期

| 文件 / 位置 | 内容与边界 |
| --- | --- |
| `vault.json` | 封装密钥头和密文状态，含确认版本、待上传状态及录入草稿；可作密文备份 |
| `unlock-cache.json` | 本机未封装数据密钥，有解锁能力；不等于保存主密码，禁止进入 Git、Drive、备份或复制到别的设备 |
| `oauth-login.json` | 使用本库密钥加密的本机登录材料；同样不进入 Git、Drive、备份或跨设备复制 |
| 仓库外 `oauth/client.json` | 完整 Google 客户端配置；不是服务密钥，也不能代替本人 Google 授权 |
| `.zprofile` 受管块 | 只有加载代码与路径，没有密钥值 |

私密库目录应为 0700、文件为 0600；可读解锁缓存的本机进程能够解密本库。`lock` 删除解锁缓存、保留密文，之后由本人 `unlock`。`disable ENTRY` 停用条目但不删除历史，也不会撤销服务商的实际 token。不要把 `lock`、`disable` 或 `logout` 当成服务商撤权。

只有确实需要明文文件时才用 `export --credential ENTRY --format json --output ABS_PATH`，本人确认目标，0600 创建且不覆盖。dotenv 对部分特殊字符会拒绝，改用 JSON 或 `run`，不要 `source` 导出文件。通常无需明文导出即可使用。
