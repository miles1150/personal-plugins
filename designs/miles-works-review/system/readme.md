# Miles Works design system · review v1

供个人多设备助手工作台使用的设计提案。继承 Waypost 当前中性色与陶土色，统一账户、消息、密钥、连接和接收诊断。不是已经上线的新品牌或生产组件库。

## 来源与方向

来源基准：personal-plugins main `1992d19dd521a1069880c7b49fdb99df2a201e87`。原始 token 取自 `services/waypost/dashboard/style.css:10`。保留背景 #f5f5f7、面板 #fff、文本 #1d1d1f、辅助文本 #6e6e73、强调 #bb4d30、深强调 #a3432a、边界 #e3e3e8、危险 #a03328、系统字体和原缓动曲线。

新增成功与待核对语义色、3 px 焦点、44 px 触达、统一 10/16 px 圆角和 4 px 间距阶梯。旧样式的 #86868b 不再承载必要的小号文字，避免低对比。主体以排版、边界和密度建立层次，不用装饰性渐变或大面积品牌色。

参考 [baoyu-design](https://github.com/JimLiu/baoyu-design) 的“读取真实样式 → tokens → 组件合同 → 交互原型 → 编译检查 → 浏览器验证”流程。上游参考 main `0fd52fc77765b214cfcc2ced20b092b2969bb0b0`，来源文件摘要见 evidence。没有运行安装脚本，没有全局安装或改变本机技能。

## 信息与文案

对用户统一称 Miles Works；安装标识仍为 miles-works。Waypost 是消息连接的部署名称，Keyenv 是密钥能力的现有实现名称，授权资源与域名中保留真实名称。Space 首次解释为消息和共享资料的协作边界；不是账户或文件夹。助手是对话身份，连接是该身份在某台设备或宿主上的访问凭据。本机接收程序首次注明 bridge。Actor、Consumer、OAuth family 只在技术详情中出现。

必须区分：已存入云端、通知已接受、助手已接收、已处理。最后一项不证明外部业务成功。客户端自行声明的名称不能写成“官方已验证”。主助手不增加权限；默认收件人和主助手是独立配置，不能混为一个 API 字段。

## Tokens 与组件

入口 `styles.css` 引入 `tokens/foundations.css` 与组件 CSS。44 个 token，组件只依赖 React 与系统 token。字体直接使用系统字体栈，无下载字体；中文行高 1.65，密集辅助信息 13 px，必要最小标签 12 px。不同平台字形会有差异。

12 个组件：Button、Field、Choice、SelectField、StatusTag、Notice、Dialog、NavigationItem、ConnectionRow、PermissionSummary、DeliveryTimeline、SpacePicker。每个目录包含 JSX、d.ts、使用说明和实际运行的 card.html。`_ds_bundle.js` 与 `_ds_manifest.json` 由参考编译器生成；`preview.html` 为生成的单文件设计系统浏览页。完整产品原型是上级目录 index.html，以同一 CSS 合同实现轻量 DOM 交互；不声称生产页面已接入这组 React 组件。

原页面已有按钮、输入／选择／勾选、导航、状态提示、dialog、连接列表与身份卡片。本次明确新增 PermissionSummary 和 DeliveryTimeline，用来准确呈现授权范围与交接语义；SpacePicker 为跨页面统一的现有选择器。没有图表、幻灯片或营销组件的虚构全量清单。

## 无障碍与响应

默认 light。没有虚构“已提供暗色模式”；新增暗色前需单独调色和验证。桌面 224 px 侧栏；中屏降低内边距；800 px 以下收起侧栏为顶部导航、消息列表与详情分屏；320 px 起无页面横向溢出。手机不丢弃安全与设备入口：它们在横向账户导航中仍可访问。

原生按钮、select、checkbox、dialog；所有字段有可见标签；错误关联字段；不只靠颜色区分；44 px 点击目标；焦点 3 px，偏移 3 px。dialog 使用 showModal，支持 Esc、焦点约束并返回原控件。消息详情保留返回列表按钮。支持减少动态效果；通知使用礼貌 live region，连续点击不重复创建多个 toast。对比、键盘与窄屏检查记录在上级 evidence/validation.md；并未做真实读屏器用户测试。

## Iconography

现有代码存在 Waypost W 标记，但本提案产品品牌为 Miles Works，不把 W 擅自改成新标志。使用纯文字字标与文本标签；Cx／Cl／Gr 为示例身份缩写，不是平台 logo。没有下载或绘制平台商标、图标字体，也不使用 emoji 卡片。小型勾号只表示可见文本已解释的状态。

## 状态要求

按钮：默认、悬停、键盘焦点、禁用、处理中防二次提交。Field：空值、有效、错误、重新登录后恢复草稿。Notice：一般、成功、需核对、阻断。密钥页：无权限、按条目读／写、创建、全量现有与未来条目、拒绝、撤销、过期与禁用。ConnectionRow：未连接、等待授权、连接完成待验证、按需接收、本机接收运行／暂停、投递未知、过期与撤销。原型与完整产品状态覆盖区别见上级报告。

## 交付索引

- preview.html：交互式设计系统评审。
- cards/：12 张基础卡。
- components/：12 个组件与 12 张示例卡。
- styles.css、tokens/：可直接被下一阶段消费的 CSS 合同。
- vendor/：固定版本 React 18.3.1 与 Babel 7.29.0，仅供离线评审，许可和来源见 evidence。
- 上级 index.html：统一产品提案原型；上级 REVIEW.md：用户旅程、取舍与下一阶段合同。

所有示例均为合成数据。设计成果等待用户评审，不标为 approved。
