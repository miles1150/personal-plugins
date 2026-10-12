# personal-plugins

个人维护的 agent 插件与配套工具集合。

## Keyenv（暂定名）

面向终端、脚本和 agent 的个人凭据工具：在设备端加密，通过 Google Drive 同步，按命令注入环境变量。

- 当前阶段：已有技术探针及正式 CLI 首轮开发实现；本机自动测试通过，真实 Google OAuth/Drive 和正式用户库尚未验收。
- [第一版设计](docs/keyenv-design.md)
- [实验工具与实机步骤](tools/keyenv/README.md)
- [验证记录](docs/keyenv-validation.md)
- 主密码由用户保管并在终端隐藏输入；不使用钥匙串，正常构建无需 Xcode。
- [本地 Mac 交接 prompt](docs/keyenv-local-handoff.md)
- CLI 与 agent 无关；插件提供操作说明和必要的分发包装。
- 仓库只存代码、文档与无真实凭据的示例。
