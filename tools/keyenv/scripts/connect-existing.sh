#!/bin/sh
set +x
set -eu
umask 077

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  printf '%s\n' '用法：connect-existing.sh Google邮箱 已有Drive文件夹ID [本机库绝对路径]'
  exit 2
fi
keyenv_account=$1
keyenv_folder=$2
keyenv_home=${3:-"$HOME/.config/keyenv/vault"}
case "$keyenv_home" in
  /*) ;;
  *) printf '%s\n' '本机库必须使用绝对路径。'; exit 2 ;;
esac
keyenv_repo=$(cd "$(dirname "$0")/../../.." && pwd)
keyenv_cli="$keyenv_repo/tools/keyenv/bin/keyenv"
keyenv_bundle="$keyenv_repo/tools/keyenv/config/oauth"
keyenv_base=$(dirname "$keyenv_home")
keyenv_profile="${ZDOTDIR:-$HOME}/.zprofile"

printf '%s\n' '使用现有客户端和 Drive 库；不创建新客户端或新云盘库。' '使用纯 Go 编译正式 CLI，不需要 Xcode。'
(cd "$keyenv_repo/tools/keyenv" && CGO_ENABLED=0 go build -o bin/keyenv ./cmd/keyenv)
mkdir -p "$keyenv_base"
if [ -f "$keyenv_home/vault.json" ]; then
  printf '%s\n' '已有本机库，仅同步，不重做 setup。'
  "$keyenv_cli" --home "$keyenv_home" sync
else
  if [ -e "$keyenv_home" ]; then
    printf '%s\n' '本机库路径已存在但缺少 vault.json；保留文件，先核查，不自动覆盖。'
    exit 1
  fi
  printf '%s\n' 'Git 中只有公开配置和密文。本人隐藏输入一次现有主密码，随后在浏览器完成授权。' '主密码不保存；新电脑独立建立本机缓存，不复制其他机器的缓存。'
  "$keyenv_cli" --home "$keyenv_home" setup --oauth-bundle "$keyenv_bundle" --oauth-client "$keyenv_base/oauth/client.json" --account "$keyenv_account" --folder "$keyenv_folder"
fi
"$keyenv_cli" --home "$keyenv_home" shell-install --profile "$keyenv_profile"
"$keyenv_cli" --home "$keyenv_home" keys --offline
printf '%s\n' '接入完成。新登录终端自动加载；当前终端可重新打开，或自行加载 .zprofile。' 'agent 使用 /bin/zsh -lc 执行程序，只核对字段存在，不打印密钥值。'
