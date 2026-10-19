# Keyenv

Keyenv 在本机加密凭据，通过 Google Drive API 同步，并向终端、脚本和 agent 提供环境变量。正式 CLI 为 `keyenv`；`keyenv-lab` 使用公开 fixture 密钥，只能处理合成数据。

## 选择正确入口

| 文档 | 适用对象与内容 |
| --- | --- |
| [通用安装](../../docs/keyenv-installation.md) | 任何用户安装预编译 CLI，配置自己的 Google 客户端，创建或接入自己的库 |
| [个人新电脑接入](../../docs/keyenv-local-handoff.md) | 本仓库维护者使用个人接入包，接入同一账号、同一客户端、同一 Drive 库 |
| [日常使用](../../docs/keyenv-usage.md) | K=V、`.zprofile`、程序与 agent、同步故障、备份恢复 |
| [开发说明](../../docs/keyenv-development.md) | 源码构建、测试、合成探针、分发与发布 |
| [验证状态](../../docs/keyenv-validation.md) | 自动化、单机实测、跨机及长期测试的证据边界 |
| [设计记录](../../docs/keyenv-design.md) | 设计背景与早期命令草案，不作安装手册 |

## 安装与数据边界

当前分发版本是实验性 [0.1.0-rc.1](https://github.com/miles1150/personal-plugins/releases/tag/keyenv-v0.1.0-rc.1)。四个平台包只带 CLI 与安装校验文件；独立的 `personal-connect` 包带维护者公开客户端配置及加密 secret。其他用户使用自己的客户端，不需要这个个人包。运行预编译 CLI 不依赖 Go、Xcode、系统钥匙串，也无需 clone 仓库。

加密格式使用 Argon2id、随机数据密钥和 AES-256-GCM。Drive 和密文备份不包含主密码、本机未封装数据密钥或 OAuth 登录缓存。setup 后缓存本机解锁密钥；Google refresh token 另以本库密钥加密保存在本机，后续自动刷新。能够读取本机解锁材料就能够解密本库，因此个人目录权限和设备访问控制仍然重要。

所有示例显式指定仓库外库路径；`--home` 必须位于子命令前，不能依赖不同系统的默认配置目录恰好相同。setup 与后续密钥维护分开，已有库无需重复 setup。

## Agent 操作说明

通用操作约束见 [Keyenv skill](../../plugins/keyenv/skills/keyenv/SKILL.md)。它是仓库内的可移植 skill 源文件，不是已安装的原生插件。CLI 可以独立使用；没有插件商店安装这一步的前提。

用户亲自进行隐藏输入和浏览器登录。agent 通过字段名、条目元数据和成功状态操作，不查看秘密、私密 JSON 或缓存内容。只有用户选择全局加载后，才使用登录 shell 继承环境；具体命令见日常使用文档。
