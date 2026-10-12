# 新电脑接续：当前有效状态（2026-10-12）

本节覆盖下方历史 prompt 中过时的阶段限制与 OAuth 状态。仓库：https://github.com/miles1150/personal-plugins 。前次实现已提交并推送至 `0d1e365`，用户报告该提交 macOS／Ubuntu CI 通过。用户已授权将本机接续修改 commit 并 push 到 main。提交前已完成本机及公开 fixture 实机验收；推送与该提交远端 CI 的实际状态由执行线程在推送后核对，不预填成功。

## 安全边界与已实现内容

- 保留全部用户未提交修改；不要 reset、强制覆盖或 force push，不重做需求访谈。
- 纯 Go 实现，模块要求 Go 1.26+。Air 已安装 Go 1.27.1；正常测试、vet 和构建使用 `CGO_ENABLED=0`，无需 Xcode。主密码由用户保管；按最新要求，setup 本人隐藏输入一次后缓存本机解锁密钥，后续维护不再询问主密码。旧库本人一次 unlock 启用；lock 删除本机缓存。主密码本身不落盘，无钥匙串或后台解锁服务。密码、验证码由本人操作，不进入聊天、argv、日志或 Git。
- 正式 CLI 已实现 init/setup/unlock/lock/add/update/disable/resolve/list/inspect/run/export/sync/status/doctor/backup/restore。vault.json 保存密文与封装密钥；独立 unlock-cache.json 保存本库未封装数据密钥（目录 0700、文件 0600），读取它即可解密本库，禁止上传、跨设备复制或放入备份/Git。OAuth token 仅存进程内。正式库与公开 fixture 的 lab 标记隔离；不得把 lab 当真实凭据库。
- 本次 Air 实际复跑通过：无 CGO 全部 Go 测试、vet、正式 CLI 与 lab 构建、simulate 六项 true。测试使用合成数据及 HTTP 模拟服务器；不代表真实 Google 验收。此前合成 PTY 的信号恢复验证见验证记录；用户额外手动密码实验已要求跳过，不重复要求。
- 已提供本机打包脚本和显式目录安装器，以及 `plugins/keyenv/skills/keyenv/SKILL.md`。不代表已注册原生插件、发布签名版本或安装到全局 PATH。

## Google 当前状态

账号 `rentlifelee@gmail.com`；Google Cloud 首次条款及 Google API 用户数据政策已获确认并接受，2SV 门槛已解除，专用项目 `keyenv-personal` 已创建并启用 Drive API。Desktop OAuth 客户端 **Keyenv Local Desktop** 已创建，无计费、免费试用或扩大 scope。

客户端 secret 曾因复制按钮的可访问性标签意外进入工具记录。用户知情后明确选择保留现有客户端，**不要删除或重建**。这里只记录脱敏事实，不记录值或原始工具记录。旧记录要求删除/重建的指令已被此决定覆盖。

上次仅检查元数据：Air 的 `/Users/mima0000/.config/keyenv/oauth/client.json` 不存在；Downloads 的旧 `a.json` 来源未核实，不能采用。手工下载尚未完成。Air 已有 0700 私密目录不代表新电脑已就绪，也不通过 Git 传递凭据。

新电脑需由用户在官方 Cloud 控制台安全下载该现有客户端 JSON 到本机仓库外私密目录（建议 `$HOME/.config/keyenv/oauth/client.json`，目录 0700、文件 0600）。用户本人打开敏感详情、下载并关闭后再恢复观察；工具只核对文件存在与元数据，不输出 JSON 内容，不用剪贴板传秘密。若当前工具可从列表直接安全下载且不会返回 secret，可在授权范围内完成；不能读取含 secret 的完整快照或标签。不要复用未核实文件。

### 当前 Mac 接续发现（2026-10-12）

本机 checkout `/Users/lizhijian/meituan/personal-plugins` 与远端均已核对为 `0d1e365`；macOS 26.5.1 / arm64 / Go 1.27.1 的无 CGO 测试、vet、正式与 lab 构建、simulate 六项、打包及临时目录安装/拒绝覆盖通过。账号和客户端已在 Cloud 列表确认；用户当次确认后，已保存唯一 `drive.file` 并添加自己的测试账号，应用仍为 External/Testing。

**下载限制修正**：Google 官方说明现有 secret 在创建后不再可查看或下载（[Manage OAuth Clients](https://support.google.com/cloud/answer/15549257?hl=en-uk)）；本机客户端详情也未找到下载入口，只确认存在 `Add secret` 操作。上一段“重新下载现有客户端 JSON”的路径不能假定仍可用。开始时本机预期 JSON 不存在；后续安全保存结果见下段。其他新设备需使用用户已安全保存的已核实配置，不要重复新增 secret。保留客户端 ID；未经另行授权不得停用/删除旧 secret、删除或重建客户端，也不得从旧聊天/工具记录恢复秘密。

用户要求由 Chrome 工具协助操作，并当次明确确认在现有客户端下新增 secret。已点击一次 `Add secret`，定位一次性 JSON 下载提示所关联的按钮并下载；浏览器下载事件超时后没有重复创建或下载，通过本机文件元数据确认新增下载（客户端 ID 匹配、当次时间、404 bytes），再以拒绝覆盖方式移到 `/Users/lizhijian/.config/keyenv/oauth/client.json`，权限 0600；父目录 0700。工具未读取/输出 JSON 内容或 secret，也未从旧工具记录恢复秘密。返回安全客户端列表后确认仍是原客户端，列表显示两个 secret；旧 secret 未停用或删除。

用户已亲自完成首个仅 `drive.file` 的 grant。真实 Drive 两次独立写入探针均退出 0；第二次明确同一实验 folder，读回首轮 fixture（`existing_fixtures_verified=1`），两次幂等检查均 true。新增 `drive-probe --read-only --page-size 1` 后，第三个独立进程只读验证两份 fixture、两页列表，`pagination_verified=true`，退出 0；未新增文件。浏览器后续复用既有相同权限，token 每进程仍只留内存，没有扩大 scope。测试文件夹 ID 及脱敏结果保存在仓库外 `acceptance-20261012/drive-probe-results.json`；仓库只记录脱敏 ID。改动后的无 CGO 全套测试、vet、lab 构建和 simulate 通过，该段为推送前验收记录；对应提交的远端 CI 须另行核对。

用户已在 Terminal 亲自完成正式 setup，正式库为 `/Users/lizhijian/.config/keyenv/vault`（0700，密文文件 0600），脱敏结果在仓库外 `acceptance-20261012/setup.json`，`cache_used=false`、`pending_uploads=0`、绑定明确正式 folder/vault。早期 `add` 曾在字段数量输入阶段失败，后续条目录入和恢复已成功，见下文；**不要重跑原 `acceptance.command` 或 setup，不另建库/文件夹**。

按用户要求，正式 CLI 提示/错误建议和用户脚本均改为中文；新条目字段数量默认 1，已有字段默认 0 保留，无效数字提示重填。`add/update --field NAME` 可预设单个字段名，并用非秘密 `--purpose/--expires` 预设元数据；字段值仍本人隐藏输入。`backup --verify-restore-home NEW_DIR --yes` 将备份与离线恢复校验合到同一进程，早期版本共用一次主密码输入；最新缓存版本无需再次输入主密码，token 仍不跨进程保存；不访问 Drive，不输出字段值，拒绝覆盖。`--yes` 只对密文 backup/restore 有效，不能跳过明文导出确认。新增恢复/默认值/无终端拒绝测试后，无 CGO 全套测试、vet、两套 CLI 构建和 simulate 通过。

入口已分离：`/Users/lizhijian/.config/keyenv/acceptance-20261012/setup-only.command` 只负责首次配置；本机 setup 已完成。用户实际运行中文 `maintenance-and-recovery.command`，公开测试条目 `keyenv-acceptance` 已保存，`maintenance-add.json` 为 `revision_saved`、`cache_used=false`、`pending_uploads=0`。第二次解锁（备份阶段）返回 `DECRYPTION_FAILED`；工具只核对元数据，确认尚未生成备份或恢复目录，不能仅凭错误代码断言密码错误或密文损坏。

**正式库恢复已通过**：用户已执行 `recovery-only.command`，返回 `backup_and_restore_verified`、`recovery_verified=true`、`confirmed_objects=1`、`pending_uploads_preserved=0`、`restored_device_changed=true`、`offline_verification=true`。旧模式 receipt 为 `unlock_material_persisted=false`、`oauth_token_persisted=false`；该事实保留为缓存功能之前的证据。密文备份及恢复目录已存在，禁止覆盖或重跑旧混合／维护／恢复脚本。当前只录入公开 fixture，没有录入实际服务凭据。

**最新解锁要求**：用户明确要求 setup 只输入一次并本地缓存，后续维护不输入主密码。这覆盖历史“每命令重新输入／不缓存数据密钥”的选择。实现缓存本库数据密钥而非主密码；默认无超时，跨进程／重启有效，显式 lock 删除。维护缺少缓存返回 `LOCAL_UNLOCK_REQUIRED`，损坏缓存拒绝使用，字段值仍本人隐藏输入。新设备 setup／restore 输入一次后建立独立本机缓存；本机恢复可用 `--source-home` 指定已解锁的同库。备份不含解锁缓存，OAuth token 仍只留内存。

新缓存实现已通过合成测试（独立进程维护、缓存权限、错库／损坏／错误 key／符号链接拒绝、lock、备份隔离及恢复），无 CGO 全套测试、vet、两套构建和 simulate 均通过。**本机旧正式库缓存实机已通过**：本人执行 `enable-cache.command` 只输入一次主密码；receipt 显示 local_unlock_cached=true、master_password_persisted=false、oauth_token_persisted=false。随后原库的 status/list --offline/run 与新路径的 backup/recovery 在独立、无终端进程中均退出 0，没有再次输入密码或 Google 登录。原库及恢复库公开 fixture 比较均 true。缓存模式恢复结果 recovery_verified=true、confirmed_objects=1、pending_uploads_preserved=0、restored_device_changed=true、unlock_material_persisted=true。目录 0700、缓存/密文/备份 0600；备份 schema 不含解锁缓存，原库密文 hash 与原缓存 inode/mtime 保持不变，之前的备份与恢复目录保留。脱敏 receipt 位于仓库外 `acceptance-20261012/cached-recovery-result.json`、`cached-acceptance-checks.json`。未读取或输出缓存内容；不重做 setup、add 或 Google 登录。Terminal 被电脑控制工具明确拒绝访问，必须本人手动启动。密码和实际服务凭据不得发到聊天或进入参数/日志。

第二台 Mac 可用，由用户稍后亲自执行；单机独立进程、分页及本机新目录恢复不计为双机验收。

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

- Google Drive 的双机一致性、其他故障场景与实际撤权；本机仅 `drive.file` 授权、真实上传/读取/幂等及只读分页已通过，见上节。
- 两台 Mac 的可见性、正式库接入、备份恢复与回读；当前 Mac 的正式库 setup、公开条目录入及密文备份/离线恢复已通过。旧正式库一次 unlock 后，无终端读取、公开值环境注入、缓存模式备份/恢复及回读均已通过；并未重新执行新库 setup 或系统重启实验。
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
