# 新电脑接续：当前有效状态（2026-10-12）

本节覆盖下方历史 prompt 中过时的阶段限制与 OAuth 状态。仓库：https://github.com/miles1150/personal-plugins 。本次已获用户授权提交并推送 keyenv 开发实现与本交接；推送/CI 的实际结果由执行线程报告，不以本文件提前声称成功。

## 安全边界与已实现内容

- 保留全部用户未提交修改；不要 reset、强制覆盖或 force push，不重做需求访谈。
- 纯 Go 实现，模块要求 Go 1.26+。Air 已安装 Go 1.27.1；正常测试、vet 和构建使用 `CGO_ENABLED=0`，无需 Xcode。主密码由用户自己保管，每次解密在本地终端隐藏输入；无钥匙串、密码缓存或后台解锁服务。密码、验证码由本人操作，不进入聊天、argv、日志或 Git。
- 正式 CLI 已实现 init/setup/add/update/disable/resolve/list/inspect/run/export/sync/status/doctor/backup/restore。磁盘保存密文与封装密钥，OAuth token 仅存进程内。正式库与公开 fixture 的 lab 标记隔离；不得把 lab 当真实凭据库。
- 本次 Air 实际复跑通过：无 CGO 全部 Go 测试、vet、正式 CLI 与 lab 构建、simulate 六项 true。测试使用合成数据及 HTTP 模拟服务器；不代表真实 Google 验收。此前合成 PTY 的信号恢复验证见验证记录；用户额外手动密码实验已要求跳过，不重复要求。
- 已提供本机打包脚本和显式目录安装器，以及 `plugins/keyenv/skills/keyenv/SKILL.md`。不代表已注册原生插件、发布签名版本或安装到全局 PATH。

## Google 当前状态

账号 `rentlifelee@gmail.com`；Google Cloud 首次条款及 Google API 用户数据政策已获确认并接受，2SV 门槛已解除，专用项目 `keyenv-personal` 已创建并启用 Drive API。Desktop OAuth 客户端 **Keyenv Local Desktop** 已创建，无计费、免费试用或扩大 scope。

客户端 secret 曾因复制按钮的可访问性标签意外进入工具记录。用户知情后明确选择保留现有客户端，**不要删除或重建**。这里只记录脱敏事实，不记录值或原始工具记录。旧记录要求删除/重建的指令已被此决定覆盖。

上次仅检查元数据：Air 的 `/Users/mima0000/.config/keyenv/oauth/client.json` 不存在；Downloads 的旧 `a.json` 来源未核实，不能采用。手工下载尚未完成。Air 已有 0700 私密目录不代表新电脑已就绪，也不通过 Git 传递凭据。

新电脑需由用户在官方 Cloud 控制台安全下载该现有客户端 JSON 到本机仓库外私密目录（建议 `$HOME/.config/keyenv/oauth/client.json`，目录 0700、文件 0600）。用户本人打开敏感详情、下载并关闭后再恢复观察；工具只核对文件存在与元数据，不输出 JSON 内容，不用剪贴板传秘密。若当前工具可从列表直接安全下载且不会返回 secret，可在授权范围内完成；不能读取含 secret 的完整快照或标签。不要复用未核实文件。

## 新电脑接续顺序

1. 先查已有 checkout、AGENTS.md 与相关 `.agents/skills`。没有 checkout 则 clone；已有 checkout 先检查状态，安全 fetch，只有无冲突且不会覆盖本地修改时才快进更新，分叉先报告。阅读本文件、[设计](keyenv-design.md)、[验证记录](keyenv-validation.md)、[CLI README](../tools/keyenv/README.md) 与 keyenv skill。
2. 核对本机 Go、CPU、路径；在 `tools/keyenv` 运行下面的无 CGO检查。新机是否通过须重新记录，不能套用 Air 结果。

```sh
CGO_ENABLED=0 go test -count=1 ./...
CGO_ENABLED=0 go vet ./...
CGO_ENABLED=0 go build -o bin/keyenv ./cmd/keyenv
CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab
./bin/keyenv-lab simulate
sh -n scripts/build-local.sh scripts/install-local.sh
```

3. 按现有脚本打包：`./scripts/build-local.sh`。安装时用户明确选择已存在的绝对 bin 目录，然后运行 `./scripts/install-local.sh ABSOLUTE_PACKAGE_DIR ABSOLUTE_BIN_DIR`；包目录以脚本实际输出为准，安装拒绝覆盖，不修改 PATH 或 shell 配置。校验和用于检查损坏，不是发布者签名。
4. 安全取得本机客户端 JSON 后，核实 External/Testing 自己的 test user 和所需 consent 配置。授权前一次说明账号与唯一 scope `https://www.googleapis.com/auth/drive.file`：允许访问应用创建或用户明确提供的文件，不是整盘权限，也不是严格单文件夹隔离；取得当次确认。只创建 `Keyenv Lab — test data only` 文件夹，读写内置合成 fixture、重复同预生成 ID 上传；密钥公开，不输入真实凭据，不初始化正式用户库。实际 Google 授权由本人完成，每进程重新授权，不存明文 token 或钥匙串。
5. 先运行 `./bin/keyenv-lab drive-probe --oauth-client 本机JSON绝对路径`，记录脱敏 folder ID 与实际结果；再用 `--folder SAME_FOLDER_ID` 独立重跑。跨设备使用同账号、同 OAuth 应用、明确同 folder ID，各自授权；必须在两台 Mac 实际读回对方数据才可记为双机通过。不要靠扩大 scope 或复制 refresh token 掩盖失败。
6. 真实 Drive probe 通过后再按 README 与用户授权推进正式 setup/终端流程；正式库不得使用 lab folder。用户亲自保管正式密码。记录真正观察到的结果与阻塞，不称全部验收完成。

## 仍未验收

- 实际 drive.file 授权、Google Drive 真实上传/读取/幂等、分页、一致性与撤权。
- 两台 Mac 的可见性、正式库接入、恢复与回读；正式用户 vault 初始化未执行。
- 完整终端 add/update/run/export、交互子进程等正式用户体验；额外旧手动密码实验继续跳过。
- 长期格式独立评审、主密码迁移、完整设备撤权、持久 OAuth 登录、原生插件注册及发布安装体验。

---

# 历史交接（供参考；以上当前状态优先）

# 本地 Mac 接续 prompt

将下面内容发给运行在 Mac 本地环境的 Codex：

```text
请接续 https://github.com/miles1150/personal-plugins 的 Keyenv 验证工作。

先定位已有仓库，保留我的未提交修改；没有则 clone。有则检查状态后更新 main，不强制覆盖。阅读 README.md、docs/keyenv-design.md、docs/keyenv-validation.md、tools/keyenv/README.md。

最新决定：主密码由我自己保管，需要解密时在交互终端隐藏输入。不使用 macOS 钥匙串，不记住主密码，不引入 CGO/Xcode 依赖，也不实现后台解锁服务。当前是 keyenv-lab 技术原型，正式 setup/add/run/export 和插件尚未完成，不要把测试成功说成产品已可用。请先完成下面的验证与阻塞修复，不重做需求访谈。

1. 检查 macOS、CPU 架构和 Go（1.26+）。正常测试及构建使用 CGO_ENABLED=0，不要求安装 Xcode Command Line Tools。若已有 C 工具链可附加 race 检查；否则跳过并记录，不为 race 单独安装 Xcode。
2. 在 tools/keyenv 执行 CGO_ENABLED=0 go test -count=1 ./...、go vet ./...、CGO_ENABLED=0 go build -o bin/keyenv-lab ./cmd/keyenv-lab，以及 ./bin/keyenv-lab simulate。
3. 引导我在自己的交互终端执行 password-probe create --file 仓库外绝对路径，然后以独立进程执行 password-probe check --file 同一路径。我输入临时测试主密码，不发给你。验证隐藏输入、两次输入不一致、正确及错误密码、已有文件拒绝覆盖。重定向 stdin 应返回 INTERACTION_REQUIRED。只记录脱敏结果，完成后清理明确的测试文件。没有交互终端时给我命令，不让密码进入聊天或工具调用。
4. 使用本地 Chrome，以 rentlifelee@gmail.com 配置 Google Drive OAuth。有浏览器控制能力就直接操作；没有就准确说明缺少哪个连接，并引导我完成页面操作。优先复用合适的自有 Google Cloud 项目和 Desktop OAuth client，否则建立 keyenv-personal 项目、启用 Drive API、配置 External/Testing 和自己的测试用户、drive.file scope、Desktop OAuth client。client JSON 下载到仓库外，不把内容输出到聊天或提交 Git。密码、验证码和账户验证由我亲自完成。
5. 运行 drive-probe --oauth-client JSON绝对路径，完成浏览器授权，记录 folder_id。再加 --folder 相同ID 重跑，验证已有 fixture 可读取及幂等上传。只用程序自带虚构数据，不录入真实 token。这个 Drive probe 使用公开 fixture 密钥，与手动密码 probe 独立；不能声称已经验证端到端主密码加密同步。OAuth token 仅存内存，每次启动需重新授权；不要擅自引入明文 token 文件或钥匙串来保持登录。
6. 有第二台 Mac 时，以同一 Gmail、同一 OAuth client、同一 folder_id 独立授权并运行，再回首台读取。没有第二台时，将跨设备验证列为待完成；单台重跑不算双机测试。
7. 修复发现的阻塞，完成适当测试，更新 docs/keyenv-validation.md，区分实际观察、自动测试与尚未验证的项目。不要把主密码、token、OAuth JSON、真实凭据或测试产物提交 Git。报告修改及结果；这次交接不要求自动推送新的修改。

请从检查本地仓库和运行环境开始执行。
```
