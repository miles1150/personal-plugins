# 旧服务退役清单与数据处置门槛

2026-10-31（Asia/Singapore）。用户明确要求删除所有旧服务，涵盖 Worker、代码和数据，并确认未迁移内容直接丢弃、不另备份。已完成下列旧云端资源删除与本地专用代码清理；未导出或输出秘密。独立旧仓库和设备残留的限制见下文。

## 执行更新：已获具体确认并完成云端删除

用户随后对“未迁移旧消息、密钥、对象直接丢弃，不迁移、不另做备份”明确回复“确认”。按此授权执行，无新增备份、无输出秘密。

- 删除两个旧自定义域名绑定、两个旧Worker，API均200成功，未使用force。
- 删除准确ID的两个旧D1数据库，API均200成功。
- 列举并删除旧R2的17个对象，确认列表为空后删除旧桶，均成功。
- 复核：Workers仅剩miles-works-production；数据库仅剩两份miles-works库；R2仅剩新桶；域名仅剩works.miles1150.top。旧两域名无剩余DNS记录、zone无Worker routes。
- 清理仅匹配waypost.miles1150.top的5条专用WAF/限流/缓存/配置规则，逐条检查expression后删除；保留zone共享与managed规则。
- 新部署仍是49a1f165-4afc-4b28-85cf-5258732fc74a，100%，与删除前一致。

以下是删除前清点快照，不是当前资源存在状态。下面的数据处置确认门槛已经满足。

剩余范围：旧独立GitHub仓库miles1150/waypost仍存在，连接器无删除仓库方法；gh只读API调用返回Forbidden，本轮未换渠道重试或删除该仓库。本机/Mac旧启动项、宿主连接、共享Google客户端的旧回调尚未获得当前可操作证据，未声称已清理。新服务共享代码不能删除；独立旧部署模板和使用文档正在本地清理，尚未合并发布。

本地代码清理现已完成：删除两份旧production模板、无调用方的独立keys profile，以及未被统一Worker构建/CI使用的旧营销网站目录（含上游旧邀请入口）。保留统一Worker依赖的keys/OAuth/消息实现、本机Keyenv、CI本地profile和历史源文档。更新当前部署/使用/回滚文档与dashboard说明。差异格式、本地文档链接、TOML解析、3项部署策略测试、dashboard静态检查、统一静态资产构建通过。代码变更尚未提交合并，不能称远端源码已清理。

## 新服务实际状态

Cloudflare 当前 `miles-works-production` 活跃版本 `49a1f165-4afc-4b28-85cf-5258732fc74a`，100%流量，部署 `a1ad13a3-1568-4a4f-ab28-a71e9e12ed82`。新域名 `works.miles1150.top` 已启用。该统一服务已部署，但本次讨论的助手/会话v2尚是方案原型。

新Worker无指向旧Worker的service binding。当前独立绑定：

- DB：`miles-works-control-production`，`34005d49-ce5e-4dfd-a391-5032370ff841`。
- KEYS_DB：`miles-works-keys-production`，`7237d31d-e230-4fb0-81a2-41cd06b1ccff`。
- OBJECTS：`miles-works-production-objects`。

这些资源均保留。本轮执行环境访问三个/health端点均遇URLError，故没有新增HTTP健康成功证据；Cloudflare配置查询成功不等于业务验收。

## 已确定的旧资源

| 类型 | 对象 | 退役影响 |
|---|---|---|
| Worker | waypost-production | 停止旧消息/API/OAuth服务 |
| Worker | waypost-keys-production | 停止旧密钥/API/OAuth服务，Worker私有secret随资源删除失去可用配置 |
| 自定义域名 | waypost.miles1150.top | 旧消息入口失效 |
| 自定义域名 | keys.miles1150.top | 旧密钥入口失效 |
| D1 | waypost-production / 15391954-8c38-492b-9856-5a90fad78142 | 删除旧消息、身份、凭据记录及其历史 |
| D1 | waypost-keys-production / 276e095b-fc6f-42f4-b4a0-88868dda10ed | 删除旧密钥条目、字段、授权记录及审计 |
| R2 | waypost-production-objects | 列表返回17个对象；仅统计，未读取内容；删除将移除旧载荷/附件对象 |

域名ID：waypost `63b18e5a1087d78864cbe116d093b6f0c2bacd50`；keys `057de5e1cd14020238f1a84e18d3e38a4f845da1`。zone `7fb57e6bfd358ed377407addb2689e7c`。新域名与zone本身不得删除。

## 新发现：并未完成数据迁移

只查询 COUNT，不读取密钥值、消息正文、凭据或OIDC代码：

| 数据 | 旧数据库 | 新对应数据库 |
|---|---:|---:|
| messages | 14 | 0 |
| key_entries | 2 | 0 |
| key_fields | 2 | 0 |
| blobs记录 | 2 | 0 |
| memory_collections | 0 | 0 |

旧消息库有18条Actor、6条OAuth family；旧密钥库有6条key OAuth family。这里只是记录数，未据此声称全部有效或活跃。新控制库Actor为1、OAuth family为0；没有真实新客户端已完成业务连接的数据库证据。

因此，旧资源不是已迁移的空壳。需要用户明确：**14条消息、2个密钥条目及旧对象，是迁移/备份后删除，还是允许直接丢弃。** 这项确认来自本轮具体发现与“新服务已全部上线”的前提差异，不是重复询问退役方向。不得因为新域名上线就默认为旧数据已备份。

## 代码范围

`services/waypost/` 是当前统一Worker使用的共享实现；`src/service/unified.rs`、keys模块、OAuth基础、消息代码、schema及bridge均不能按旧名称整目录删除。`tools/keyenv`也需要逐调用链核查，不能把本地工具都视为旧部署。

真正可清理的候选包括：独立旧Worker部署模板/发布路径、已失效旧URL文档和安装示例、专用旧产品入口，以及确认无新服务调用的兼容分支。先做引用和编译检查，形成可审阅diff，保留新统一服务必须使用的代码；重命名共享目录是另一可逆整理，不等于删除功能。

旧独立Waypost仓库、Mac启动项/bridge账本、宿主旧连接、Google旧回调、DNS/WAF专用规则尚未逐一核验。不删除整个Git历史、不删除Google客户端或共享规则；它们可能仍服务新域名。此处是已知剩余清点范围，不声称“所有旧物已找齐”。

## 获得数据处置决定后的执行顺序

1. 更新源码/文档的退役清单与发布引用；检查新统一构建和关键业务测试。
2. 如选择迁移，先安全迁入新库并验证密钥可解密、消息及附件可读；不能直接搬密文跨AAD/keyring。备份必须包括可恢复性，不能只保存无法解密的数据库。
3. 停旧发送入口和旧bridge调度，确认没有在途未知操作；如明确选择直接丢弃则记录此影响。
4. 删除旧专用域名/Worker，再删除准确ID的旧D1和旧R2对象/桶；每步验证，不使用通配或force删除依赖。
5. 复核新Worker版本/绑定/域名/登录及消息密钥操作保持正常；处理可访问设备旧连接/启动项。
6. 删除独立旧部署代码并完成必要发布；未获访问的设备/仓库条目保留明确未完成项，不假称已清空。

当前未执行步骤3–6；等待上述旧数据具体处置确认。
