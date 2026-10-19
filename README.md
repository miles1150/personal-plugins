# personal-plugins

个人维护的 agent 插件与配套工具集合。

## Keyenv

面向终端、脚本和 agent 的凭据工具：本机加密，通过 Google Drive API 同步，以环境变量供程序使用。支持隐藏输入 K=V，以及用户选择启用的登录 zsh 自动加载。

| 你的目标 | 从这里开始 |
| --- | --- |
| 其他用户安装 CLI，使用自己的 Google 客户端和密钥库 | [通用安装](docs/keyenv-installation.md) |
| 本仓库维护者在新电脑接入已有个人库 | [个人新电脑接入](docs/keyenv-local-handoff.md) |
| 录入、更新、同步、让程序或 agent 使用密钥 | [日常使用](docs/keyenv-usage.md) |
| 开发、测试、实验探针、发布 | [开发说明](docs/keyenv-development.md) |
| 查看已验证与待验证范围 | [验证状态](docs/keyenv-validation.md) |

CLI 的安装和使用方法可复用；个人接入包只服务于维护者自己的 OAuth 客户端与现有库，不是其他用户共用的登录服务。通用安装不需要个人接入包，也不需要知道维护者的主密码。

当前提供实验性 [0.1.0-rc.1 预编译包](https://github.com/miles1150/personal-plugins/releases/tag/keyenv-v0.1.0-rc.1)，支持 macOS/Linux 的 arm64/amd64，无需 Go、Xcode 或钥匙串。单机真实 OAuth、Drive、录入及恢复已有验证记录；双 Mac 与长期授权仍待验收，不能据此宣称稳定版。

主密码只由本人在终端隐藏输入，不保存主密码。仓库可分发代码、文档、公开客户端配置和经授权的客户端密文；完整 client.json、服务密钥、OAuth token、本机解锁与登录缓存不进入 Git。

更多入口见 [Keyenv README](tools/keyenv/README.md)。CLI 不依赖 agent 品牌；仓库中的 [skill](plugins/keyenv/skills/keyenv/SKILL.md) 是可移植操作说明，尚不代表原生插件已经注册。
