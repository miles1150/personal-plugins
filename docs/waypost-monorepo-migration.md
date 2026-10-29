# Waypost 单仓迁移与切换

## 仓库归属

Waypost 服务、共享 MCP 协议层、OAuth、D1 迁移、Worker 配置、bridge 和原项目测试位于 `services/waypost`。插件入口位于 `plugins/waypost`、`plugins/keyenv` 和 `plugins/keyenv-remote`；bridge 的离线市场打包在 `tools/waypost`。密钥 CLI 位于 `tools/keyenv/cmd/keyenv-remote`。代码迁移本身不会部署 Worker、修改 Cloudflare D1/R2、域名、已签发凭据或 Google OAuth 客户端。

初始导入是原仓库 `miles1150/waypost@11c4668f63ff0ef2c67026ff12c061d6c7ac980f` 的完整已提交工作树快照，Git tree 为 `54de379acc706f002fe275cf0716ea2af2ccb1ed`。导入提交只记录快照，不把原仓库全部历史合并进本仓库；原仓库和 PR #17 保留，可查阅完整历史。导入快照包含已整合 #15/#16 的 #17 代码；正式切换前须检查迁移分支后续提交以及原仓库是否有新的必需提交，不能假定两个仓库仍同步。

插件 bridge 当前与原 0.3.0 绑定的 `9f70cbf` 相同，其 bridge Git tree 为 `3cb14e1b29a3f628166cb054a4c780a29d041a92`。单仓 CI 从提交的 `services/waypost/bridge` 构建，校验该树、平台架构和产物 SHA-256；制品记录原来源和单仓提交。现有 marketplace Git pin 和现有安装不因服务代码迁入而变更。更新 bridge 时需同步更新 pin、版本、manifest 和复验流程。

## PR 与 CI

- 迁移分支包含 personal-plugins #6 的客户端提交和 Waypost #17 的代码快照；不要再直接合并原 Waypost #17，避免重复引入迁移和 OAuth 实现。迁移 PR 合并后核对 #6 的提交是否已进入目标分支，再根据实际状态处理 #6；不删除原 PR 或仓库。
- #5 是尚未完成真实端到端验收的 `miles-works` 草稿，应以迁移后的 main 为基础重新整合。测试合并显示四个文本冲突：市场文档、两份插件测试和 bridge 打包脚本；两个市场索引也要确认保留 `miles-works`、`waypost`、`keyenv` 和 `keyenv-remote`。新统一入口是否包含远程密钥 MCP 和 skill 需明确实现及测试，不能只添加索引。发布草稿前仍需解决其原有 bridge 认证和真实宿主验收限制。
- `.github/workflows/waypost-ci.yml` 从单仓服务目录运行原服务测试、Wasm 和 Worker 检查，并加入合成账号的两仓 CLI/native 与 Worker/D1 集成；`.github/workflows/waypost-bridge.yml` 构建 bridge 制品。`plugin-checks.yml` 从仓内固定 bridge tree 构建市场制品，不再 clone 旧服务仓库。原服务目录里的 workflow 副本已迁至仓库根，避免无效副本漂移。

## 切换前核对

1. 在非生产环境完成单仓 CI、真实客户端及升级验证。核对部署脚本和工作目录指向 `services/waypost`；确保 `wrangler` 使用已核实的现有 account、Worker 名称、D1 和 R2 绑定、路由及同一 `PUBLIC_ORIGIN`。源码迁移不能改变 OAuth issuer 或自动撤销授权。
2. 生产 D1 先做可恢复备份并验证；核对已应用的 `0015_oauth_client_registry.sql` 名称与内容，绝不覆盖或重复应用。新代码依赖 `0016_keys.sql`，应先在隔离副本验证升级、现有授权及数据保留，正式库按已确认的变更窗口应用，再部署；回滚要保留已迁移的数据与原有密钥。
3. 新密钥服务使用独立 D1 和 Worker，先核实 Google issuer/subject，按 `services/waypost/docs/design/0017-gateway-cimd-dcr-keys.md` 准备配置与离线备份；设置持久凭据、创建云资源、域名绑定和正式部署均为实际操作，须遵守操作时的确认要求。密钥服务真实 Google 登录、撤销、Cloudflare 限流/日志以及 Claude、Codex、Cursor 和 cloud session 授权复用仍须验证。
4. 比对生产健康检查、现有账号、Space、消息、授权、接收与发送流程，再决定切换发布来源。不要删除、归档旧仓库，或撤销旧部署/凭据，直到切换与恢复已验证且另有授权。
