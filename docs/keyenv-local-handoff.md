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
