# Miles Works 插件市场

当前源码和新版市场只保留 `miles-works` 一个插件，包含一个远程 MCP（`https://works.miles1150.top/mcp`）和三个技能：消息、远程密钥、本地 Keyenv CLI。版本为 0.6.0。两宿主的清单是同一插件的兼容文件，不是两个产品。

## 安装和本人登录

发布后使用 `distribution/miles-works-v0.6.0`；合并后的 main 索引固定到该分发提交。不要用旧市场缓存判断新版本。

```sh
codex plugin marketplace add miles1150/personal-plugins --ref distribution/miles-works-v0.6.0
codex plugin add miles-works@personal-plugins
claude plugin marketplace add 'miles1150/personal-plugins#distribution/miles-works-v0.6.0'
claude plugin install miles-works@personal-plugins
```

已有同名市场时先核对来源及已装插件，使用宿主支持的市场更新／替换流程，不直接修改缓存。安装后检查版本、一个 MCP 和三个技能；用户本人打开 Connect／登录，审阅消息、密钥读取、修改及新建权限。安装不会自动授权或启动 bridge。新站 Google 浏览器登录已在 Mac 验证，宿主登录和业务工具验收仍是独立步骤。

## 旧入口清理与恢复

新版删除 `plugins/waypost`、`plugins/keyenv`、`plugins/keyenv-remote` 的独立包装和两份市场中的对应条目，技能保留在 Miles Works 内。`services/waypost`、`tools/keyenv`、旧 Worker/D1/R2、密钥库、OAuth 家族和 Actor/Consumer 不受影响。未删除历史分发引用；可从清理前提交 `8d55c95` 恢复旧源码。用户设备的旧插件缓存不会随 Git 删除自动卸载。

用户已允许卸载多余插件。设备执行者应先确认目标宿主与旧插件 ID，保留宿主配置、私有凭据和 bridge 账本，再用宿主插件卸载命令移除旧包装。不要调用 OAuth revoke、删除凭据文件、清理数据库或卸载 Keyenv CLI。若旧 bridge 正在运行，先核对其状态和切换计划，避免重复唤醒或丢失未处理工作。

## 分发与兼容范围

源码插件没有二进制；CI 分发含 Linux x86-64、Mac ARM64、Mac Intel 的 bridge。`skills/waypost/scripts/bridge.py doctor` 校验来源树、构建提交、平台和摘要；不会运行接收服务、下载程序或回退到 PATH。实际运行须用户明确配置并授权。插件升级／卸载不应删除其缓存之外的账本和凭据。

CI 工件 `personal-plugins-market` 是可离线安装的市场包；若向 Plugin Creator 上传一个插件，应只打包其中 `plugins/miles-works/`，不能把整个市场当成单插件档案上传。Git 分发树只含该插件、两个宿主市场索引和 `MILES-WORKS-RELEASE.json`，不再继承旧主分支工作流。发布检查运行于该记录中的源码提交和 workflow run，而非分发树。

兼容依据、已验证宿主版本、目录上架限制和可恢复性见 [结构审查](miles-works-plugin-audit-2026-10-30.md)。本次没有提交 OpenAI 或 Anthropic 公共目录，也没有新增 Cursor 兼容承诺。
