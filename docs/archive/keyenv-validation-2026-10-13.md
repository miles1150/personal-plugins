> 历史归档：保留截至 2026-10-13 的原始检查点，各段状态只代表当时，不能作为当前安装或操作指令。当前请从 [文档入口](../../tools/keyenv/README.md) 开始。不要重跑文中的一次性验收脚本。

# Keyenv 第一轮验证记录

日期：2026-10-11（历史首轮）；最新状态：2026-10-13。

## OAuth 客户端分拆与加密 Git 分发（2026-10-13）

用户明确授权分拆客户端配置：公开白名单明文、client_secret 加密后 Git 分发；不能沿用历史“所有客户端配置绝不入 Git”阻止这次授权。已实现正式 client-pack 和新设备 setup --oauth-bundle，CLI 在进程内处理秘密，只报告元数据。实际本机现有 JSON 打包退出 0：同一个 client ID、client_secret_encrypted=true、roundtrip_verified=true、source_client_unchanged=true，oauth_tokens_included=false、unwrapped_key_published=false。公开文件仅 client_id/project_id/Google接口/loopback回调地址；未知字段拒绝。密文带 KDF/封装 key header，不包含未封装 key 或任何登录/服务 token。未改现有账号、scope、Google客户端或令牌，未发 Google 请求。

本机无 CGO 全套 Go 测试、vet、正式/lab 构建、simulate 六项以及中文接入脚本 sh -n 通过。新增合成测试：错误密码不生成 client.json；同主密码重建相同客户端，0600/0700且幂等；原输入不改变；密文/公开文件不含测试秘密或密码；陌生客户端/源中的 token/未知字段/非官方端点/非loopback拒绝；公开元数据改动、同时改 hash、伪造密文/库头、交换配对拒绝；Git目录及其符号链接别名拒绝写明文；既有不同客户端/链接不覆盖；bootstrap禁止新建云盘库、无本人终端拒绝、已有库在授权前拒绝重做。实际客户端密文的缓存 key 回读不等于 Air 主密码/浏览器验收；后者仍待本人执行，两个阶段准确分开。

## 本批最终修复与实际字段注入（2026-10-12）

用户要求 commit/push 本批。首次 set 失败时仅公开 fixture、待上传为 0，输入未保存。未确认具体请求和外部网络原因。已修复提交空行后先加密保存整批计划，重新创建上传/OAuth context，不把用户录入计入网络超时；网络失败时保留草稿并由 sync 恢复，显示进度。新增合成用例实际通过：Reserve 不可用／重开／加密备份恢复保留整批；草稿不进入确认环境；上传响应丢失保留同对象、重试不重复；确认成功但 draft checkpoint 未记录时重放不创建新版本；父版本变化拒绝覆盖；discard 不清已确认数据。草稿位于绑定密文内，0600，不存明文参数或独立 plaintext 文件。最新无 CGO 全套 Go 测试、vet、两套构建与 simulate 六项通过。

第二次本人终端录入成功，keys_saved、待上传 0。实际独立新登录 zsh 与其子进程验证：OPENBB_TOEKN／CLOUDFLARE_API_TOKEN／DEEPSEEK_API_KEY／GROK_WEBHOOK_URL／GROK_WEBHOOK_AUTH_HEADER 五者 present=true、nonempty=true；只输出布尔值，未显示字段值，未验证服务商认证或执行 webhook。实测某默认 agent 执行器的直接进程环境没有这五个变量；显式 /bin/zsh -lc 才加载，证据范围限定于登录 shell/其子进程。脱敏真实字段结果保存在仓库外 real-fields-injection-checks.json。用户曾在聊天发出实际秘密，已建议本人轮换；这些内容未写入工具参数/源码/日志/Git。双 Mac 与跨周 Google 登录仍未实机验收。下面“只录入公开 fixture／真实字段未录入”均为此前时刻，不代表当前状态。

## K=V 与登录 shell 自动加载验收（2026-10-12）

用户本次明确授权 classic K=V + `.zprofile` 自动全局注入。新增 set/keys/env/shell-install，未写入真实服务凭据。纯 Go 全套测试、vet、正式/lab 构建、simulate 六项通过；相邻 OpenBB helper 11 项测试通过。新增合成测试覆盖首个等号分割、空值/大小写/字面值、非法与系统变量拒绝、旧服务分组保留其他字段、不同新 key 独立条目、重复/冲突/禁用拒绝；真实 zsh 进程确认引号/换行/美元/命令替换片段不求值、不创建 marker，xtrace 不出现合成 secret，返回后 trace 状态恢复、nounset 兼容，重载移除先前加载字段。安装测试确认原 profile/后续自定义文本/权限保留、0600 备份、幂等、标记错误/符号链接拒绝。额外 CLI 测试确认无终端 env/keys/profile 使用缓存、secret 参数和无明确 offline zsh 格式的 env 提前拒绝。

本机 `.zprofile` 已实际安装（原内容完整保留，0644，备份0600），重复安装 backup 为空。独立非交互登录 zsh 的子进程仅输出比较布尔值：公开 fixture 匹配 true、keyenv/keyenv_load 可用，OPENBB_TOEKN/CLOUDFLARE_API_TOKEN/DEEPSEEK_API_KEY 尚未配置。profile 标记块不含 fixture 值；启动期间本地密文/解锁/OAuth 文件 inode/mtime 未改变；未显示 Google 或主密码提示，没有读取解锁/token 文件内容。脱敏 receipt 保存为仓库外 shell-profile-checks.json。此次只验收公开测试条目加载，未亲自完成新 set 的真实服务录入或业务 API 请求；不代表所有已经运行的 GUI/agent 进程环境已更新。新代码未提交/推送，远端 CI 和第二台 Mac 仍待验证。

## 加密 OAuth 缓存开发与实机验收（2026-10-12）

用户明确授权正式 CLI 加密保存 refresh token、自动刷新，保留现有客户端和 drive.file，不用钥匙串。随后选择继续 Drive API，保留 Testing；Production 和网页部署不再推进，目录/Git/R2 后端未实现。首次真实持久授权已由本人完成；独立进程缓存复用、真实 Google refresh 及联网读取通过。实现详见 README；新代码尚未提交或推送，基线 4d8aeb0 的 CI 成功不能覆盖本次修改。

- 无 CGO 全套测试通过：首次 offline/PKCE/单 scope；合成回调选错账号不持久化；HTTP 刷新及 refresh token 轮换、后续 token source 复用、并发只刷新一次；invalid_grant/503/扩大 scope/写入失败保留旧 cache，不泄露错误响应；令牌接口 307 不转发凭据。缓存 AES-GCM ciphertext 不含合成 token/账号/密码明文，0600；跨会话 Load、错账号/客户端/设备/header、篡改、公共权限、符号链接、过大输入拒绝；加密备份和恢复不含登录材料，logout 保留 vault。
- vet、正式/lab 构建、simulate 六项 true 通过，未安装 Xcode 或运行依赖 CGO 的本机 race。真实授权由本人完成并退出 0；随后非交互独立进程 login 复用、login --refresh 真实 Google 刷新、doctor 校验当前账号/库/header、在线 list 均退出 0，没有新的授权或主密码输入。refresh 后登录文件 inode/mtime 改变（原子替换）、文件 0600、原解锁缓存 inode/mtime 未变。未读取令牌内容或回调 URL。实际显式续期通过，自然到期检测由合成测试覆盖；跨小时/跨周与真正撤权仍未实机执行，不宣称长期登录已验证。
- 登录缓存存在时的新路径密文备份/恢复实际通过，源库登录保持有效、oauth_token_in_backup=false、restored_oauth_cache_state=absent，恢复目录不存在 oauth-login.json，confirmed_objects=1、新设备 ID 正确、备份 0600。私有 oauth-cache-live-checks.json、oauth-cache-recovery-checks.json 及各步骤脱敏结果保存于仓库外 acceptance-20261012。旧备份和恢复目录均保留。
- Google 发布状态实际仍为 Testing，按钮禁用并指向缺失的 Branding。首页与隐私政策为空；用户提供的 openbb-bot.miles1150.top/docs 是 OpenBB Swagger，含 OpenBB 名称和 example.com/terms 占位链接，不是 Keyenv 主页/隐私。此前准备三份专用静态源码；用户当前选择继续 Drive API 并保留 Testing，网页草稿不部署。未上线、未填写未验证 URL、未切换 Production、未更改客户端或 scope。

## 新 checkout 接续验收（2026-10-12，当前 Mac）

- 本机仓库为 `/Users/lizhijian/meituan/personal-plugins`。开始时工作区干净；安全 fetch 后 `main` 与 `origin/main` 均为 `0d1e3653ce4a030ee55a4760442958ffb0452fc1`，无需合并或覆盖。仓库及已检查的父目录没有 `AGENTS.md`，仓库没有 `.agents/skills`；已阅读最新交接、设计、CLI README 和 keyenv skill。
- 本机为 macOS 26.5.1（25F80）、darwin-arm64，与此前 Air 的实机记录分别计数。开始时未安装 Go；通过现有 Homebrew 的官方 core 预编译 bottle 安装 Go 1.27.1，没有安装 Xcode 或启用 CGO。
- 本机实际通过：`CGO_ENABLED=0 go test -count=1 ./...`、`CGO_ENABLED=0 go vet ./...`、正式 CLI 与 lab 的 arm64 构建、simulate 六项 true、两个脚本的 `sh -n`、本机打包、专用临时目录安装和已有文件拒绝覆盖。产物留在 Git 忽略的 `bin/`、`dist/` 或临时目录，没有安装到全局 PATH。这些是本机合成/HTTP 模拟证据，不替代真实 Google 或恢复验收。
- Cloud 控制台实际确认账号为 `rentlifelee@gmail.com`，项目为 `keyenv-personal`，现有 Desktop 客户端为 **Keyenv Local Desktop**；始终未删除或重建该客户端。配置最初为 External/Testing、零测试用户、空 scope 列表。用户当次明确确认后，已保存唯一 `https://www.googleapis.com/auth/drive.file`，并在测试用户列表确认该账号（Google 显示大小写为 `RentlifeLee@gmail.com`）。未发布应用，未扩大 scope。
- 仓库外 `/Users/lizhijian/.config/keyenv/oauth` 已以 0700 权限准备。用户随后要求 Chrome 工具协助；工具仅检查非敏感按钮文字，详情未找到下载入口，确认有 `Add secret`。Google [官方说明](https://support.google.com/cloud/answer/15549257?hl=en-uk)创建后的 secret 不可再次查看或下载，旧“去详情下载”指引已修正。用户当次确认后，在同一个客户端下只新增一次 secret 并点击一次下载。浏览器下载事件未返回成功，随后通过本机当次下载文件的元数据（客户端 ID 匹配、时间、404 bytes）确认文件已存在，未盲目重复操作。文件以拒绝覆盖方式移至 `/Users/lizhijian/.config/keyenv/oauth/client.json`，权限 0600；未读取或记录 JSON 内容。安全客户端列表确认原客户端仍保留且有两个 secret；旧 secret 未停用或删除，没有从旧聊天/工具记录恢复秘密。
- **真实 Google 已通过**：用户亲自完成首个仅 `drive.file` 的 grant，首轮 `drive-probe` 退出 0，创建实验文件夹并上传/读回合成密文，同 ID 重复上传的幂等检查为 true。第二个独立进程在相同文件夹（脱敏 ID：`1Rug…y9sa`）读回首轮 fixture，`existing_fixtures_verified=1`、幂等检查 true，退出 0。浏览器确认既有权限，无新增 scope；每进程 token 仍只存内存。没有输出主密码、验证码、OAuth JSON 内容或回调中的授权码。
- **真实分页已通过**：新增实验命令选项 `--read-only --page-size 1`，只读取明确实验文件夹。第三个独立进程真实输出 `existing_fixtures_verified=2`、`list_pages=2`、`pagination_verified=true`、`read_only=true`，退出 0；未新增对象。该证据仅代表此刻本机完整读回两页，不证明双机互读或全球即时一致。
- 分页探针改动后再次通过无 CGO 全套测试、vet、lab 构建和 simulate。新增 HTTP 测试验证请求页大小、分页 token、完整成功的页数、无效选项在授权/联网前拒绝、失败页不能返回部分成功。正式 `List` 默认仍为每页 100，原有安全上限保持；未跑本次新增修改的远端 CI。
- **正式 setup 已通过**：用户亲自在 Terminal 隐藏输入主密码并完成正式 setup；脱敏 `setup.json` 显示明确正式 folder（`19F-…FUL7`）、vault（`5d24…191e`）、`cache_used=false`、`pending_uploads=0`、最近成功拉取时间及两项不持久化标记。本地目录 0700、`vault.json` 0600；未读取用户密码或磁盘密文内容。早期 `add` 在字段数量输入阶段返回 `INVALID_DATA`，`add.json` 为空；后续条目录入与恢复已成功，见下文；保留原正式库，不重跑 setup 或另建文件夹。
- **实机输入阻塞与默认值修正**：先修正字段数量失败；随后按用户要求将正式终端提示、错误建议和浏览器登录说明改为中文。新条目默认 1 个字段（1..64），已有字段默认 0 保留，无效数字提示重填。增加 `add/update --field NAME` 及非秘密 `--purpose/--expires`，单字段场景只询问隐藏字段值，跳过其余默认元数据问题。真实秘密值不接受 argv/env，也没有默认值；单字段普通交互可修改默认变量名。
- **恢复输入减少**：新增 `backup --verify-restore-home NEW_DIR --yes`，早期版本在同一进程内共用一次隐藏输入的主密码；最新缓存版本不再输入主密码，生成密文备份、恢复到新目录、比较完整解密版本/待上传密文/绑定并验证新设备 ID；不访问 Drive或输出字段值。`--yes` 仅限密文 backup/restore，不适用于明文 export，新设备无缓存恢复仍需本人一次隐藏输入，始终拒绝覆盖。新增合成测试覆盖含已确认／待上传对象的独立磁盘恢复、原库不变、报告／备份无明文、已有目标拒绝及错误密码不报成功；全部无 CGO 测试、vet、两套 CLI 构建、六项 simulate 通过。
- **中文维护流程实际运行**：用户执行 `maintenance-and-recovery.command` 后，`maintenance-add.json` 实际为 `revision_saved`、`credential=keyenv-acceptance`、`cache_used=false`、`pending_uploads=0`，正式库已保存公开测试条目。备份恢复阶段第二次解锁返回 `DECRYPTION_FAILED`，脱敏进度为该阶段退出 1；工具元数据检查确认备份文件与恢复目录均不存在，恢复结果文件为空。可确认失败发生在原库解锁、生成备份之前；不能仅据错误代码断言密码错误或密文损坏。
- **正式库恢复实际通过**：用户已运行 `recovery-only.command`，程序返回 `backup_and_restore_verified`、`recovery_verified=true`、`offline_verification=true`、`confirmed_objects=1`、`pending_uploads_preserved=0`、`restored_device_changed=true`、`ciphertext_only=true`。仓库外 `recovery-only-result.json` 保留脱敏结果，进度为 complete／exit 0。`encrypted-backup.json` 与恢复目录的 `vault.json` 均为 1749 bytes／0600，原库与恢复目录均 0700。此运行属于旧不缓存模式，两项 persisted 标记为 false；没有 Google 登录或再次录入条目。禁止覆盖已有备份、恢复目录或重跑早期脚本。
- **最新缓存模式（用户明确要求）**：setup 输入一次主密码，保存本机未封装数据密钥；后续维护用缓存，不再提示主密码。旧库一次本人 unlock 建缓存，lock 删除；master password 不保存，OAuth token 仍只存进程内。`unlock-cache.json` 绑定本库 ID/header 摘要，0600，私有目录 0700；缓存具有解锁能力，不上传、不进密文备份/Git、不跨设备复制。维护无缓存返回 `LOCAL_UNLOCK_REQUIRED`，不自动要求密码；损坏／不安全缓存返回错误。
- **缓存合成验证已通过**：重新打开本库可用同一 data key；缓存不含 master password／条目字段；密文备份不含缓存 key。覆盖缓存过大／损坏／公开权限／符号链接／错 header／错误 key 拒绝和错误后释放锁，lock 留存密文并要求 unlock；同库无密码恢复、错库拒绝、已有目标拒绝、完整版本/绑定/待上传比较。CLI 无终端调用 status/list/inspect/backup／恢复通过；首次 init/unlock/新设备 restore 仍需要本人终端。全套无 CGO 测试、vet、两套构建、simulate 六项通过；这是提交前本机证据；用户随后授权 commit/push，对应提交及远端 CI 状态由推送后实际核对。
- **缓存实机已通过**：本人执行 `enable-cache.command`，只输入一次旧库主密码；receipt 为 local_unlock_cached=true、unlock_material_persisted=true、master_password_persisted=false、oauth_token_persisted=false。本机目录 0700、unlock-cache.json 0600，工具仅检查元数据，从未读取缓存内容。原库 status/list --offline/run 分别在独立非交互进程退出 0，公开 fixture helper 返回 fixture_matches=true、restored_fields_available=true、synthetic_value_only=true，没有再次输入密码。
- **缓存模式恢复实际通过**：新路径 `cached-encrypted-backup.json`／`cached-restored-vault` 的 backup --verify-restore-home --yes 在无终端进程退出 0，无主密码输入或 Google 登录。receipt 为 backup_and_restore_verified、recovery_verified=true、confirmed_objects=1、pending_uploads_preserved=0、restored_device_changed=true、offline_verification=true、ciphertext_only=true、unlock_material_persisted=true、oauth_token_persisted=false。恢复库在后续独立无终端进程 status/list/run 全部成功，公开 fixture 比较 true。备份与密文/缓存均 0600，恢复目录 0700；密文备份 schema 不含缓存材料；原库密文 hash 和原解锁缓存 inode/mtime 保持不变，之前的备份/恢复目录保留。私有 `cached-recovery-result.json` 和 `cached-acceptance-checks.json` 保存脱敏证据。本次没有重做新库 setup、add、系统重启或联网更新，不将这些步骤计为新缓存实机验收。旧混合／维护／恢复脚本已完成其用途，不要重跑或覆盖产物。
- **仍待执行**：两台 Mac 互读与正式库接入/恢复、实际撤权及其他一致性场景、实际服务凭据及完整维护体验。Terminal 被电脑控制工具明确拒绝访问（应用被禁止控制），必须由用户手动启动。第二台 Mac 可用，由用户稍后亲自执行。旧额外手动密码实验按交接继续跳过。


## 提交交接前最新复核（2026-10-12）

Air / Go 1.27.1 / darwin-arm64 实际复跑通过 `CGO_ENABLED=0 go test -count=1 ./...`、`CGO_ENABLED=0 go vet ./...`、正式 CLI 与 lab 构建、simulate 六项 true。均为本机合成测试，无真实 OAuth 或 Drive 授权。没有安装 Xcode，未复跑可选 race。另已通过打包/安装脚本语法、本机打包、临时目录安装与拒绝覆盖、CLI 命令清单、skill frontmatter/安全指引检查及 `git diff --check`；本次候选 29 文件的凭据模式扫描无命中（扫描并非完整泄漏证明）。构建产物和临时安装未进入 Git。

Google Cloud 条款、API 用户数据政策及 2SV 已完成；keyenv-personal 的 Drive API 已启用，Keyenv Local Desktop 已创建。用户知情后明确保留现有客户端。客户端 JSON 上次仍不存在，Downloads 旧 a.json 未核实；手工下载、真实 drive.file 授权、实际 Drive 与双机/正式用户 vault 验收仍待完成。没有计费或免费试用。新电脑接续以 [最新交接](../keyenv-local-handoff.md) 为准，下方旧检查点描述保留为历史。

## 正式 CLI 首轮实现检查点（2026-10-12）

用户最新明确授权按现有设计实现正式功能，已覆盖此前仅限探针的范围；下文旧交接阶段限制保留为历史记录。没有运行用户真实初始化，没有输入用户正式密码、API key 或 OAuth token。

本次新增 `cmd/keyenv`、`internal/app` 和 `internal/terminal`：空本地初始化、复用 wrapped-key header 的终端解锁、加密绑定/密文状态保存、macOS/Linux 进程锁、Drive setup/明确 folder 接入、add/update/disable/list/inspect/resolve、run 环境注入、交互确认导出、sync/status/doctor、加密备份及新目录恢复。正式 revision 使用 `keyenvRevision`/`keyenvHeader`/`keyenvVaultFolder` 专用标记，拒绝把实验文件夹或公开 fixture key 当正式库。OAuth 改为 online 请求，token 只留进程内，不保存 refresh token；真实运行每次仍需用户浏览器授权。

实现边界：沿用原 Argon2id/AES-256-GCM primitive 与封装；初始化和导出不覆盖，原子完整文件发布；绑定信息加密验证；已有库持进程锁。交互更新提交前复核所见 heads，拒绝看不见的并发变更。显式 resolution 字段允许用户选择版本合并同名分支，并保留所有原始对象；旧原型不保证读取该新增字段的兼容性。初始化先持久化预生成 folder/header ID，再创建远端对象；响应丢失复用同 ID 校验恢复，不另建替代库。dotenv 对可能被不同加载器改变的字符串安全拒绝，任意字符串改用 JSON 或直接环境注入。

**本次实际通过**（Mac arm64、Go 1.27.1、CGO_ENABLED=0）：全套 `go test -count=1 ./...`、`go vet ./...`、`cmd/keyenv` 与 `cmd/keyenv-lab` 两个 Mach-O arm64 构建、simulate 六项 true、正式 CLI 非终端 init 返回 `INTERACTION_REQUIRED` 且不创建目标目录、`git diff --check`。CI 增加正式 CLI 构建，但未推送或运行 GitHub CI。

新增自动测试证据：初始化/备份/恢复/导出拒绝覆盖；错误密码及解锁失败后释放锁；0600/0700 与公开文件拒绝；主密码、合成凭据名/值和绑定账号/应用不在磁盘明文出现；备份脱离 Drive 恢复且更新 device ID；待上传跨进程恢复、同 ID 重试不重复；损坏本机缓存/绑定不作为离线数据；NUL 拒绝；JSON 字面值往返、dotenv 歧义拒绝；参数/环境字面传递、退出码 17 和信号退出码 143；交互编辑远端变化拒绝；同名显式解决保留所有历史；production/lab Drive 标记隔离、账号查询和唯一 header 校验；folder 创建响应丢失后同 ID 409 恢复。HTTP 测试均是合成服务器，不是真实 Google。

**OAuth 当前真实观察**：Chrome 可操作，2SV 门槛已解除；已有 Default Gemini Project 没有 OAuth client。已创建专用项目 `keyenv-personal`（ID 同名），已启用 Google Drive API。应用表单准备为 Keyenv Personal、External/Testing、支持/联系邮箱 rentlifelee@gmail.com，停在新增 Google API 服务用户数据政策复选框等待当次确认。尚未提交 OAuth 应用配置、创建 Desktop client、下载 client JSON 或授予 Drive 访问；未开试用或计费。

**未验收/下一步**：接受新 API 用户数据政策后完成 consent、test user、drive.file 数据范围及 Desktop client；凭据创建和真实授权依规定取得当次确认，秘密由用户本地输入。运行两次真实 drive-probe，再用户亲自执行正式 setup。完整终端 add/update/run/export 与信号转发/交互子进程实机体验、第二台 Mac、真实 Drive 一致性/撤权/分页、格式独立评审、插件包装/安装器仍待完成。本次没安装 Xcode，未跑可选 race。此检查点不是正式产品已验收。

## 本地 Mac 接续检查（2026-10-12）

- 实际环境：macOS 26.5.2（25F84）、arm64；本地已安装 Google Chrome。
- 未发现已有 personal-plugins checkout，因此在独立工作目录 clone。`main` 与 `origin/main` 均为 `3967a7a`，包含手动密码解锁变更；clone 后工作区干净，没有覆盖或重置用户修改。
- 仓库中没有 `AGENTS.md` 或 `.agents/skills`；已阅读本地交接、设计、验证和工具 README。
- 本机 PATH 及检查的常见工具目录没有 Go。官方发布元数据可读取，选择的 Go 1.27.2 darwin-arm64 归档 SHA-256 为 `76812b213b1b2302c978d28fa52fa92d541704b9e7d9d5db8002c50e4018c4c5`，但尚未下载成功或安装。
- 下载阻塞：go.dev 跳转到 dl.google.com 后 curl TLS 连接失败，直接重试同样失败；官方 storage.googleapis.com/golang 路径返回 HTTP 403。Python TLS 栈也因缺少可信签发证书失败。没有关闭证书验证、使用第三方镜像或安装 Xcode。
- 下载诊断：系统 UTC 时间为 2026-10-11，系统代理及 HTTP(S)_PROXY/ALL_PROXY 均未设置。获网络审批后 dl.google.com 可解析并建立 TCP，但在 TLS ClientHello 后失败，没有 HTTP 状态；不是自动审批拒绝。Python 默认 CA 文件不存在，另有证书信任环境问题。所有尝试均保留 TLS 校验。
- 恢复：Homebrew 官方 formula 元数据可访问，`brew install --force-bottle go` 成功。其本机 formula 版本为 Go 1.27.1（满足 1.26+），安装 arm64_tahoe 预编译 bottle，无运行依赖；未安装 Xcode。Go 编译器与 CGO/C 工具链是不同依赖，移除 CGO 不会移除编译 Go 源码的需求。GitHub releases API 返回空列表，未发现仓库发布的预编译产物。
- **本次 Mac 通过**：`CGO_ENABLED=0 go test -count=1 ./...`、`CGO_ENABLED=0 go vet ./...`、无 CGO 构建；产物是 Mach-O arm64。`simulate` 六项均为 true；真实本机进程重定向 stdin 返回 `INTERACTION_REQUIRED`（退出码 1）。这是自动化/非交互证据，不能替代用户隐藏输入体验。
- 已准备仓库外专用交互脚本，并通过 `zsh -n`；已在本地 Terminal 打开，等待用户亲自验证不回显、确认不一致、独立进程正确/错误密码、拒绝覆盖及 0600 权限。脚本只保存脱敏状态并清理明确 fixture；不记录输入、不开启 shell trace。
- 后续状态复核：Go 1.27.1 位于 `/opt/homebrew/Cellar/go/1.27.1`，`/opt/homebrew/bin/go` 已在当前 PATH；实验 binary 位于 `tools/keyenv/bin/keyenv-lab`，未安装为全局 `keyenv-lab` 命令。无需重复安装。实验进程仍运行，0600 脱敏结果文件仅有首个隐藏输入提示、没有验证结果，当前等待用户第一步操作；未重启或读取输入。读取 Terminal 窗口标题的 AppleScript 未返回，已取消，不据此猜测确切系统窗口标题。
- **未运行**：真实用户手动密码验证、Google OAuth、实际 Drive 和双 Mac 测试；可选 race 未运行，未为其安装 C 工具链。
- 下一步由用户完成已打开的本地 Terminal 隐藏输入实验，再使用 Chrome 配置 OAuth。未创建 Google Cloud 项目或 OAuth client，未授权任何权限，也未生成密码 fixture 或 OAuth token。

### 用户调整后的手动实验与 OAuth 状态

用户随后表示已操作，并明确要求跳过剩余手动密码验证，不追加或重复实验。现有脱敏日志只证明 `password-probe create` 成功，输出 `synthetic_data_only=true` 和 `unlock_material_persisted=false`。脚本首步标签为 mismatch，但实际退出码为 0 并创建文件，因此不能将确认不一致记为通过。独立进程正确/错误密码、实际不回显、拒绝覆盖和 fixture 权限的用户体验检查均记为**用户要求跳过**，不是失败或通过。明确的仓库外测试 fixture 已删除；不读取密码或输入历史，不重启交互窗口。

已开始 Google OAuth 准备：本地 Chrome 页面确认登录 `rentlifelee@gmail.com`；浏览器扩展连接失败，改用本机 UI 控制。Downloads 和预期 keyenv 配置目录未找到 Desktop OAuth client JSON（仅记录数量，不展示内容）。Google Cloud 首次使用显示服务条款弹窗、国家/地区当前为新加坡；停在接受条款之前，等待用户亲自处理或当次明确确认。未启动免费试用、设置计费、创建项目或 OAuth credentials，也未请求 Drive 权限。后续原型权限仍限定 `drive.file`，token 仅存进程内，真实 Drive 与双 Mac 验证仍未运行。

用户阅读具体条款链接并当次明确确认后，已接受 Google Cloud 条款，保留新加坡地区、营销邮件未勾选。页面确认已进入控制台。全部项目列表只有 `Default Gemini Project`（`gen-lang-client-0278756149`）；仅选择它作只读检查，没有修改 Gemini 项目配置。进入 OAuth 凭据列表被重定向至 Cloud 的 MFA 门槛页面，显示账号必须启用两步验证才能继续。已有 OAuth client 尚未能核实。认证设置更改交由用户亲自在本地 Chrome 完成；不读取或代填密码、验证码、备份码。当前未创建项目或 credentials、未启用 Drive API、未进行 Google OAuth 授权、未开通试用或计费。

### 等待账号验证期间的原型修正

交接要求先完成探针验证及阻塞修复，正式 setup/add/run/export 是明确尚未完成的后续功能。本次没有新增正式初始化命令，也没有生成用户正式 vault。正式本地初始化的后续最小计划：复用 `vault.Create` 与现有隐藏输入；选择仓库外私有目录；独占创建、不得覆盖既有 vault；原子保存现有格式 header 和初始密文状态；独立进程解锁并验证损坏/错误密码/权限边界。Drive 接入时需另行绑定明确 vault/folder/account/application，不能把固定 fixture key 当真实数据密钥。开始这项后续实现前应明确正式初始化范围与格式评审要求。

本次实际改动：Drive probe 在读取 OAuth 配置或授权前显示“fixture 密钥公开、只上传内置虚构数据、不使用主密码或初始化正式库”；成功 JSON 增加 `fixture_key_public=true`、`master_password_used=false`。README 对应说明没有用户数据导入参数，禁止将固定 fixture 改成真实凭据。已有文件夹检查保持原行为；新增 HTTP 合成测试覆盖专用标记、普通文件夹、错误标记、标记为实验的普通文件、已删除文件夹和损坏元数据，并确保检查只读取所选文件夹元数据。

修改后 Mac Go 1.27.1 实际通过无 CGO 全部单元/HTTP 测试、vet、构建、simulate（六项 true）。使用不存在的 OAuth 配置路径做无网络入口检查，确认先显示公开密钥提醒、随后返回 `AUTH_REQUIRED`（退出码 1）；未发生浏览器授权或远端写入。上述是模拟及本机无账号检查，真实 Drive/OAuth、双 Mac 验收仍待用户两步验证及后续授权。


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

macOS 与 Google 实机步骤见 [实验工具 README](../../tools/keyenv/README.md)。返回结果只需包含是否成功、错误代码、系统版本和是否出现授权提示，不要附 token、主密码或 OAuth client 文件内容。

## 手动主密码变更验证

已移除整个原生钥匙串包及 `keychain-probe`，替换为 `password-probe create/check --file PATH`。当前不持久保存主密码、数据密钥或 OAuth token，没有跨进程自动解锁。

本次在 Linux amd64、Go 1.27.2 实际通过：

- `CGO_ENABLED=0 go test -count=1 ./...`、`go test -race -count=1 ./...`、`go vet ./...`。
- 无 CGO 本机构建，`simulate` 六项均为 true。
- `CGO_ENABLED=0 GOOS=darwin GOARCH=arm64` 及 `amd64` 交叉构建；不等于已在 Mac 运行。
- 密码 fixture 单元测试：正确密码、错误密码、对象绑定篡改、尾随 JSON 拒绝，序列化内容不含测试密码或合成明文。
- Linux 伪终端端到端实验：输入不回显、确认不一致不写文件、新进程重新解锁、错误密码拒绝、文件权限 0600、已有文件拒绝覆盖、管道输入返回 `INTERACTION_REQUIRED`。仅使用公开合成测试密码，临时文件已清理。

Mac 实际终端体验、Google OAuth、真实 Drive 及双 Mac 可见性仍未执行。密码实验和 Drive fixture 实验各自独立，尚未实现完整的手动主密码凭据库产品流程。CI 配置已增加无 CGO 测试及构建；此记录不代表 GitHub CI 已完成。

### Follow-up security and packaging checkpoint

- Read-only review identified hidden-input signal restoration and duplicate child signal forwarding; both corrected. `run` now closes the vault session and replaces the CLI process with the target using `exec`, retaining native terminal signals and exit status.
- Synthetic PTY tests on this Mac verified SIGINT/SIGTERM during hidden input restore ECHO, return 130/143, and create no vault. These are automated synthetic tests, not user-password acceptance.
- Fresh `CGO_ENABLED=0 go test -count=1 ./...` passed with authorized loopback access; sandbox-only run could not bind httptest's local port. `go vet`, build, and whitespace validation passed.
- Import initialization failure now removes only the newly created empty directory.
- Added host-local build package and explicit-destination installer, tested in a temporary directory including overwrite refusal. No global install or shell configuration modification performed. Checksums detect accidental corruption, not publisher authenticity.
- Added portable `plugins/keyenv/skills/keyenv/SKILL.md`; native plugin registration is not claimed.
- Google API user-data policy acceptance remains pending. No OAuth Desktop client, credential JSON, token grant, real Drive sync or real user vault initialization completed at this checkpoint.
- Offline cache freshness and complete rollback detection are outside current guarantees. Byte-slice clearing is not a guarantee of complete Go process-memory erasure.

### OAuth policy checkpoint

User explicitly confirmed the Google API Services User Data Policy through the parent thread. After Air was unlocked, accepted that policy in the existing Chrome `keyenv-personal` configuration flow and saved the prepared application branding. Google Cloud displayed “已创建 OAuth 配置！” and the overview explicitly showed no OAuth clients configured. Opened the client creation form without submitting. Desktop credential generation, credential download, test-user/scope access changes and actual Drive authorization remain pending their specific action-time confirmation. No billing/trial activated and no secret read or entered.

### Desktop client creation incident checkpoint

The user approved creating the Desktop client and saving its JSON outside Git. Created `Keyenv Local Desktop` in `keyenv-personal`. During UI control discovery, filtering for button labels inadvertently included Google's copy-secret accessibility button label, exposing the client secret in tool output. No credential JSON was downloaded, no Drive grant obtained, and the credential was not used. Closed the secret dialog. The user was informed and subsequently explicitly chose to retain this client. That decision supersedes the earlier proposed deletion/recreation; do not delete or recreate it. The new client secret itself is deliberately not recorded here.

## 预编译分发自动验证（2026-10-13）

干净 main `7b1d84e` 上独立分支实现，Linux amd64 / Go 1.26.8 本地：无 CGO 全套测试、race 全套测试、vet、正式/lab 构建、simulate 六项、shell 语法及 diff 检查通过。四平台 `CGO_ENABLED=0` 交叉构建完成，候选版本 `0.1.0-rc.1`；只在 Linux amd64 执行 binary。通用四包和独立个人接入包均按文件白名单检查，整体压缩包 SHA256、payload 校验及 binary/VERSION/包名一致验证通过。

分发测试使用临时 HOME 和限制 PATH（没有 Go、shasum），验证空格/相对/不存在路径、SHA256 篡改、清单额外条目/路径穿越、相同重复安装、不同安装拒绝/显式替换、符号链接拒绝；失败保留旧安装。使用现有外部 bundle 但无终端接入，只到 INTERACTION_REQUIRED，未解密客户端、未授权 Google、未创建 client.json/解锁或 OAuth 缓存/.zprofile。本地 build-local 安装与幂等重试通过。此环境默认 umask 077 和 /tmp 注入 .git 会导致现有 profile/client-bundle 测试失败；复测改为 umask 022、隔离 /dev/shm 临时目录后通过，没有修改安全限制。

macOS 两架构及 Linux arm64 仅交叉构建，Air 离线，首次本人接入、主密码/浏览器授权、双机同步及真实登录 zsh 加载仍待实机验收。未绕过 macOS 安全警告、未提交私人明文、未合并/推 tag/发布 Release。远端 CI 结果以 PR 检查为准。

### PR review 修复

version 已移到全局参数解析之后，支持登录 zsh 包装器前置的 `--home ABS_PATH version`，不打开或创建 vault；新增 bare/空格路径/等号形式及非法参数回归测试，并在真实分发 binary 安装测试中验证。lab CI 改为循环逐个 `sh -n`，避免 glob 后只检查首文件。发布清单新增公开 `BUILD-COMMIT` 并由顶层 SHA256 覆盖，分发测试核对它与构建 checkout SHA 一致。

本地相关及全套无 CGO/race/vet、两套构建、simulate、逐文件 shell 检查和四平台分发测试通过。首次完整复测出现合成 OAuth callback 失败，原样重跑无 CGO 全套与 race 后通过；未修改 OAuth 实现或接触真实授权。

实际尝试 workflow_dispatch 未启动：CLI 获取 workflow 返回 Forbidden，connector 确认 main 尚无该文件（404）；未擅自合并。为直接验证发布 workflow，增加只构建上传 artifact 的 PR 入口，版本为 0.1.0-rc.1，发布 job 仍只允许 tag push。PR artifact 中 BUILD-COMMIT 记录实际 checkout（GitHub PR merge SHA），不能误认为 workflow_dispatch 已验收。最终运行和可下载 artifact 链接以本 PR 新 SHA 的 Actions 为准。
