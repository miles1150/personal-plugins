# Keyenv 第一轮验证记录

日期：2026-10-11。

## 环境与范围

本轮在 Linux amd64 云端工作区运行。使用 Go 官方发布元数据指定的 SHA-256 校验安装的 Go 1.27.2；模块最低 Go 1.26，依赖版本固定在 go.mod/go.sum。

Google Drive 通过 HTTP 测试服务器验证适配逻辑，双客户端同步通过内存故障存储验证；没有登录用户 Google 账号，没有创建远端测试文件，没有导入真实 token。

实际执行结果：`go test -race -count=1 ./...`、`go vet ./...`、构建与 `keyenv-lab simulate` 全部通过。`FuzzOpen` 进行了 120,053 次输入实验并通过；这只是有界短时模糊测试，不等于密码学审计。离线模拟输出的六项检查均为 true。

## 已通过的自动化场景

- 正确/错误主密码、密文篡改、对象 ID / vault ID 替换、未知格式、KDF 参数与输入大小上限。
- 两个客户端并发更新同条目保留双方，冲突阻止选用；不同条目并发可共存，同名新条目不能静默合并。
- 停用阻止选用；多父版本能表示显式解决冲突；缺失父版本、循环和跨条目父关系被拒绝。
- 密文缓存可离线读取，新离线修改被拒绝；认证过期可使用已有缓存，损坏对象不能触发离线回退。
- 上传成功但响应丢失后，待上传记录仍保留；重启后同 ID 重试不会新增重复对象。
- 未确认上传的修改不会作为默认运行版本；保存本机状态失败时不会上传。
- Drive multipart 编码、分页、409 后校验相同内容、拒绝不同内容、拒绝其他文件夹对象。
- Drive 部分页失败、重复分页 token、重复对象、incompleteSearch、过大响应及 HTTP 错误分类。
- 本地密文状态原子替换、0600 文件权限、拒绝公共可读目录或缓存文件。
- 脱敏摘要不包含测试秘密字段值；持久化状态不包含条目名称或秘密原文。
- 原钥匙串接口已移除；新的手动密码验证结果见下方变更记录。

## 尚未验证

1. macOS 实机隐藏输入、错误密码和跨进程重新解锁体验。
2. 同一 Gmail 与同一个 OAuth 应用在两台 Mac 的实际 drive.file 可见性、Google 实际预生成 ID 上传及 409 行为。
3. Google OAuth 跨周刷新：原型只维持进程内登录，不持久化刷新令牌。
4. 实际 Drive 延迟可见性和账号撤权；自动测试的错误模拟不能替代真机结果。
5. 正式 CLI 的环境变量注入、文件导出、进程锁、信号传递、备份恢复和安装体验：这些功能尚未实现。
6. 密码格式独立评审、长期兼容性、主密码变更和完整设备撤权。

## 复现

在 `tools/keyenv` 内运行：

```sh
CGO_ENABLED=0 go test -count=1 ./...
go vet ./...
CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
```

macOS 与 Google 实机步骤见 [实验工具 README](../tools/keyenv/README.md)。返回结果只需包含是否成功、错误代码、系统版本和是否出现授权提示，不要附 token、主密码或 OAuth client 文件内容。

## 手动主密码变更验证

已移除整个原生钥匙串包及 `keychain-probe`，替换为 `password-probe create/check --file PATH`。当前不持久保存主密码、数据密钥或 OAuth token，没有跨进程自动解锁。

本次在 Linux amd64、Go 1.27.2 实际通过：

- `CGO_ENABLED=0 go test -count=1 ./...`、`go test -race -count=1 ./...`、`go vet ./...`。
- 无 CGO 本机构建，`simulate` 六项均为 true。
- `CGO_ENABLED=0 GOOS=darwin GOARCH=arm64` 及 `amd64` 交叉构建；不等于已在 Mac 运行。
- 密码 fixture 单元测试：正确密码、错误密码、对象绑定篡改、尾随 JSON 拒绝，序列化内容不含测试密码或合成明文。
- Linux 伪终端端到端实验：输入不回显、确认不一致不写文件、新进程重新解锁、错误密码拒绝、文件权限 0600、已有文件拒绝覆盖、管道输入返回 `INTERACTION_REQUIRED`。仅使用公开合成测试密码，临时文件已清理。

Mac 实际终端体验、Google OAuth、真实 Drive 及双 Mac 可见性仍未执行。密码实验和 Drive fixture 实验各自独立，尚未实现完整的手动主密码凭据库产品流程。CI 配置已增加无 CGO 测试及构建；此记录不代表 GitHub CI 已完成。
