# Miles Works · 助手与工作会话方案

2026-10-31（Asia/Singapore）｜领域与交互提案；另含已执行的旧服务退役记录。

后续更新：领域模型仍为提案；用户确认后另行执行了旧独立服务的云端退役，详见 RETIREMENT.md。新统一服务未重新部署。UI已按最新精简原则修订，新增24项针对交互和3项自包含HTTP检查；旧71项为此前版本证据，不表示当前界面重跑了旧矩阵。账户入口右上角，挂载归助手，模拟控件默认折叠。

先看 [交互原型](prototype/preview.html)，再看 [完整领域模型](DOMAIN.md)、[分阶段改造方案](IMPLEMENTATION.md) 和 [待决定规则](QUESTIONS.md)。preview.html 是已内嵌依赖的单文件；index.html 是开发入口，推荐通过 HTTP 预览。

```sh
python3 -m http.server 4313 --directory designs/miles-works-domain-v2/prototype
```

在运行服务器的环境中打开 `http://localhost:4313/index.html`。下载整个包后也可在本机用上述方式预览。所有演示数据都是合成数据；不连接 Google、Cloudflare 生产、真实客户端或秘密。

单文件的 HTTP 加载、语言切换与零外部网络请求已验证。当前浏览器管理策略阻止 file URL，所以没有把双击本地文件列为已验证；未修改浏览器策略。

## 推荐模型

账户拥有助手。助手拥有唯一业务权限、固定消息地址和收件箱；客户端挂载到助手，以它的身份工作。工作会话属于助手、运行在某客户端中，不单独隔离权限。默认会话可为空；收件人既可以是助手，也可以是助手下公开注册的会话。Consumer、token family、分配和租约留在内部实现。

原始收件目标与实际执行会话分别保存。通知、读取、领取和处理完成分别记录。bridge 是本地接收组件，不是另一个助手；若独立挂载就继承助手权限。bridge 实例与被唤醒的 Codex 实例分开，通过已验证的会话唤醒绑定关联。

## 内容与优先级

| 文件 | 用途 |
|---|---|
| DOMAIN.md | 定义、关系、生命周期、权限、会话路由、默认删除、恢复不变量 |
| IMPLEMENTATION.md | 当前源码差距、数据/API、Cloudflare部署边界、迁移回滚、P0–P5、真实验收 |
| QUESTIONS.md | 只收集尚未确定的行为，不重复已经确认的模型 |
| prototype/index.html | baoyu-design 双语交互原型；助手/权限/挂载/会话/收发/桥接恢复 |
| prototype/DESIGN.md | 旅程、组件合同、未实现能力及验证范围 |
| research/*.md | 两路当前代码研究，含证据路径及当时评估选项 |
| reviews/*.md | 四路审阅原始发现及后续复核 |

冲突时以修订后的 DOMAIN/IMPLEMENTATION/QUESTIONS 为方案基线；research 和 reviews 保留探索历史，不把旧建议自动当最终决定。QUESTIONS 中推荐值供演示使用，仍待用户确认。

开发首先验证无助手 OAuth 接入→批准挂载→同凭据调用的宿主支持，然后实现助手鉴权和会话级路由，再接 bridge；最后才迁移与部署。不要先隐藏旧页面字段后声称模型完成。

## 证据与限制

- 实时远端 main：`95d5830253923bdac6afe8cb147ce1fc21f3dcc1`；工作区基线：`5cd69447a61f4ec002b1aab5df9d79c0c7089889`。后者包含未合并的账户 UX 变更，不等于生产。
- 使用 [JimLiu/baoyu-design](https://github.com/JimLiu/baoyu-design) 本地版本 `0fd52fc77765b214cfcc2ced20b092b2969bb0b0` 的 baoyu-design 技能；复用现有 Miles Works 设计系统并实际导入组件。没有重复采纳旧的权限/Space 表单模型。
- 领域、授权迁移、bridge、UX 四路子 Agent 审阅；主要发现与处理见 [审阅处理记录](reviews/RESOLUTION.md)。代码审阅不等于安全测试或宿主验收。
- 原型初轮71项检查通过；末次改动另跑5项视觉/文案和8项解绑/替补检查，均零运行错误，没有把原71项重新运行的结果混入末次记录。独立UX复验7项，另有单文件打包加载检查。包含中英两语言、390/768/1440视口；axe仅最后工作会话页面指定WCAG规则无违规，不是全站无障碍认证。记录见 prototype/*validation.json、axe-results.json 和 reviews/ux-review.md。它们不证明 v2 协议、服务端、Google、Mac mini 或 Grok 已可用。
- 本轮只新增方案目录；没有修改业务实现、合并 PR、创建持续授权、迁移用户数据或部署生产。

## 本轮采用的自主默认

不新增连接业务ACL，不做thread权限隔离，不做无限制共享资料权限，不自动合并旧grant，不重放unknown任务，不删除旧服务/数据。安装ID、路由CAS、续租和fencing由工程设计承担，不让用户配置。

注册是否默认展示、默认替补范围、对话连续性及显式改投、积压接管，以及共享资料是否一并改造，集中在 QUESTIONS。其余必要授权留到实际操作时说明具体对象和影响。
