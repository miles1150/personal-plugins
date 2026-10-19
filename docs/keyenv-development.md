# Keyenv 开发与分发

安装用户从 [通用安装](keyenv-installation.md) 或 [个人接入](keyenv-local-handoff.md) 开始。本页只用于源码开发、合成探针和维护者分发，不是新电脑使用密钥的前置步骤。

## 源码构建与检查

需要 Go 1.26+。从仓库根目录进入 `tools/keyenv`：

```sh
cd tools/keyenv
CGO_ENABLED=0 go test -count=1 ./...
CGO_ENABLED=0 go vet ./...
CGO_ENABLED=0 go build -o bin/keyenv ./cmd/keyenv
CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
for keyenv_script in scripts/*.sh; do sh -n "$keyenv_script"; done
```

程序为纯 Go，普通构建和测试无需 Xcode、C 编译器或 macOS SDK。可选 `go test -race -count=1 ./...` 需要 CGO 与 C 工具链。`simulate` 完全离线，只用内存存储和公开合成数据模拟两台设备；每项布尔结果不等于双机实测。

本机打包与显式安装：

```sh
./scripts/build-local.sh
# 使用上一步输出的绝对 PACKAGE_DIR，BIN_DIR 必须是已存在的绝对目录：
/bin/sh scripts/install-local.sh ABSOLUTE_PACKAGE_DIR ABSOLUTE_BIN_DIR
```

产物位于忽略的 `dist/`，源码 binary 位于忽略的 `bin/`，都不提交 Git。开发包版本为 `dev`。安装器不修改 PATH 或 profile；同一 binary 幂等，不同 binary 需明确 `--replace`，符号链接与校验失败拒绝。

## 独立实验探针

`keyenv-lab` 不维护真实凭据。尤其 `drive-probe` 的加密密钥是源码公开 fixture，不能写入任何真实服务密钥，也不作为正式库初始化步骤。

隐藏输入实验使用专门的测试密码，在本人控制的终端执行：

```sh
./bin/keyenv-lab password-probe create --file /absolute/private/NEW-password-fixture.json
./bin/keyenv-lab password-probe check --file /absolute/private/NEW-password-fixture.json
```

`create` 隐藏输入并确认，0600 新建且拒绝覆盖；`check` 在新进程重新输入。错误密码返回 `DECRYPTION_FAILED`，输入不一致为 `PASSWORD_MISMATCH`，没有交互终端为 `INTERACTION_REQUIRED`。该探针不使用正式 CLI 解锁缓存。不要重跑历史个人验收脚本或把正式主密码输入探针。

Drive 实验需要本人已批准的 Desktop client JSON（仓库外），以及仅 drive.file 的本人浏览器授权：

```sh
./bin/keyenv-lab drive-probe --oauth-client /absolute/private/oauth/client.json
# 使用上一步返回的实验 folder ID；第二个进程或第二台机器：
./bin/keyenv-lab drive-probe --oauth-client /absolute/private/oauth/client.json \
  --folder LAB_FOLDER_ID
# 只读分页校验，已有至少两个 fixture 时可观察多页：
./bin/keyenv-lab drive-probe --oauth-client /absolute/private/oauth/client.json \
  --folder LAB_FOLDER_ID --read-only --page-size 1
```

首条创建明确标注的 `Keyenv Lab — test data only` 文件夹；写合成密文、读回并用同 ID 上传检查幂等性，不自动清理。只有带实验标记的文件夹可复用，绝不填正式库 ID。第二个进程应读到已有 fixture；双机时再返回第一台检查对方新增数据。只读模式不上传；`pagination_verified=true` 只说明这次确实读到多页。

lab 登录 token 仅在进程内，和正式 CLI 的加密 refresh token 缓存不同。不要扩大 scope、更换客户端或复制其他设备 token 掩盖可见性问题；测试完仅清理本人明确识别的实验文件夹。

## OAuth 客户端加密包维护

这是维护者已有库授权的可选分发方式，不是通用安装依赖。已解锁的正式库可将现有客户端打包到新的输出目录，不重做 Google 登录、不修改源 JSON：

```sh
./bin/keyenv --home /absolute/private/keyenv-vault client-pack \
  --oauth-client /absolute/private/oauth/client.json \
  --output /absolute/private/NEW-client-bundle
```

`client.public.json` 只允许 client_id、project_id、Google 官方端点和 loopback 回调等公开白名单；`client.secret.enc.json` 包含 AES-GCM 密文、公开配置摘要和封装密钥头，绑定客户端、库 ID 和原 header。不包含明文数据密钥、密码、服务密钥或 OAuth token。未知字段拒绝公开，完整 JSON 拒绝写入 Git checkout。

只有这对公开配置和密文经过本人授权后可分发。更换 client secret、库 header 或主密码后应重新生成匹配的两文件；不要只改公开文件。Git 历史会保留旧密文，不能将“更新文件”视为撤销旧副本。新设备仍须本人隐藏输入密码及浏览器授权；不手工解密打印，也不复制设备缓存。

## Release 包与自动化

从 `tools/keyenv` 执行，输出必须是新的绝对目录：

```sh
./scripts/build-release.sh keyenv-v0.1.0-rc.1 /absolute/new/keyenv-release
```

这条命令只准备包，不推 tag、不发布。需要检查下列白名单及校验结果：

| 产物 | 内容 |
| --- | --- |
| 四个 `keyenv-VERSION-OS-ARCH.tar.gz` | `keyenv`、`install-local.sh`、`checksums.sh`、`VERSION`、内部 `SHA256SUMS` |
| `keyenv-VERSION-personal-connect.tar.gz` | `connect-existing.sh`、`checksums.sh`、`VERSION`、内部清单、公开客户端配置和加密 secret |
| `BUILD-COMMIT` | 实际构建 checkout 的公开 Git SHA |
| 顶层 `SHA256SUMS` | 所有五个压缩包和 BUILD-COMMIT 的哈希 |

个人配置在 binary 外部，不将整个仓库打包。版本同时写入 binary、VERSION 和包名；校验和不是签名。`scripts/test-distribution.py` 的参数与场景见脚本源码；CI 执行四平台构建和分发校验。

[lab workflow](../.github/workflows/keyenv-lab.yml) 在 Ubuntu/macOS 运行测试、race、vet、构建、simulate 和逐脚本语法检查，并在 Linux 检查分发。[release workflow](../.github/workflows/keyenv-release.yml) 的 PR 与 `workflow_dispatch` 只准备 Actions artifact；推送 `keyenv-v*` tag 才由独立有写权限的 job 创建 GitHub prerelease，同名 Release 不自动覆盖。

截至 2026-10-19，`keyenv-v0.1.0-rc.1` 已实际发布。后续发布需按当次授权核对审核提交、CI、版本、白名单及校验后再推新 tag；文档更新或普通 main push 不等于要求发布新版本。GitHub prerelease 与 Google OAuth Testing/Production 是两个独立状态。验证边界见 [验证状态](keyenv-validation.md)。
