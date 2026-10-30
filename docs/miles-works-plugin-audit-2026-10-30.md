# Miles Works 插件结构审查

## 结论与依据

采用 Plugin Creator 的 `create-plugin/references/plugin-packages.md`、`local-plugins.md` 和 `zip-requirements.md` 审查：Git 管理插件继续从本仓库发行，不另建账户插件。便携根清单已有正确 Agent Plugins 1.0 schema，技能位于直接子目录并有匹配的 YAML 名称与描述，MCP 配置不带凭据。保留 `.claude-plugin/plugin.json` 与 `.mcp.json` 作为同一插件的 Claude 兼容文件。当前 Codex CLI 实测能读取根清单，不需要再增加 `.codex-plugin` 副本。

对照 [Claude 官方 manifest 文档](https://code.claude.com/docs/en/plugins/manifest-reference)：插件元数据位于 `.claude-plugin/plugin.json`，技能等组件放在插件根目录的标准位置，根 `.mcp.json` 会被发现。当前结构符合这一模型。便携 `streamable-http` 与 Claude 的 `http` 是各自清单中的传输表示，不能删掉其中一个文件后假定另一宿主会自动读取。

[Claude 官方市场文档](https://code.claude.com/docs/en/plugins/marketplace-reference)支持相对路径和 Git 子目录来源。已组装的分发市场采用相对路径；源码市场用完整 SHA 固定到带二进制的分发提交。后者在同仓库不同分发树上使用 Git 子目录是本项目的发行选择，且已由实际宿主安装验证，不是单凭 schema 推断。

社区对照采用作者自己的 [Superpowers manifest](https://github.com/obra/superpowers/blob/main/.claude-plugin/plugin.json) 和 [Anthropic 示例仓库](https://github.com/anthropics/claude-code/blob/main/plugins/README.md)：可以由一个插件聚合多项技能，并保留宿主标准清单。这里只借鉴文件布局和版本管理，不导入其 hooks、权限或工作流程。没有发现必须为每个技能另建插件的要求。

## 最终结构

```text
plugins/miles-works/
  plugin.json                 # Agent Plugins 1.0
  mcp.json                    # 一个 Miles Works MCP
  .claude-plugin/plugin.json  # 同名、同版本，Claude 兼容
  .mcp.json                   # 同一地址，Claude transport
  skills/waypost/             # 消息及可选 bridge
  skills/keyenv-remote/       # 统一 MCP 下的密钥工具
  skills/keyenv/              # 独立本地 CLI 的操作说明
  UPSTREAM.md
  UPSTREAM-LICENSE
```

版本 0.6.0，两份市场仅有一个 `miles-works` 条目。构建直接读取该目录，不再把旧插件拼接成伞形包装。已删除三个旧包装，保存上游许可证并修正旧来源树说明。历史 Git 提交和分发引用仍可恢复这些文件；没有改动云服务、数据、凭据、连接或本地安装状态。

## 校验与明确限制

本地使用 Codex 0.159.2、Claude Code 2.1.312。Claude 的普通严格格式校验通过；CI 另使用固定 Claude 2.1.296，检验两个版本的实际兼容性。隔离宿主测试检查安装、一个 MCP、三个技能、缓存版本升级及 loopback 合成 OAuth；不登录用户账户。三个平台的可执行文件仍须经过 CI 构建、摘要及 launcher 验证。

Claude 2.1.312 还输出独立的公共目录 profile 诊断：缺少整包 LICENSE／license、目录图标和隐私页面。普通安装格式通过不代表公共目录可提交。本次仅 Git 发行；保留现有上游 Apache 许可证，不擅自为全部本地技能声明新的许可证，也不编造隐私 URL。若将来提交公共目录，另完成这些材料。

0.5.0 分发树继承了旧 main 的工作流，其中残留 0.4.0 路径。实际 0.5.0 发布检查运行在源码 `23556499545f7f934566bccfffa9f2361a3af5f5`，run `41006637604`，随后也从 Git 引用安装验证了 0.5.0。新分发改为只包含插件、市场和来源记录的独立根树，消除这类歧义；最终发布 SHA 和 CI 结果记录在分发文件中。

## 设备迁移

用户已要求删除多余插件包装；这不等于撤销已有身份授权。设备执行者先核对旧插件 ID、运行中的 bridge 及缓存外账本，再用宿主卸载旧包装。保留现有连接凭据、旧 Worker、Keyenv 库和 CLI。用户本人进行 Miles Works 登录与授权；工具业务验收须待用户完成后进行。源码和市场只剩一个插件不代表设备上旧缓存已经自动消失。

## 最终分发

0.6.0 已发布到 `distribution/miles-works-v0.6.0`（`9a7c81fb7249292de005dfa62694249e89366fe7`），源码 `c1ea7a418352270ab3cf3bff86740d5de8c9e59a`，发布检查 [41012221333](https://github.com/miles1150/personal-plugins/actions/runs/41012221333) 全部通过。两份 main 市场索引只保留该版本；旧不可变分发仍可用于恢复。
