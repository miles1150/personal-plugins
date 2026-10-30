# Miles Works · 助手优先交互原型

最新版按用户UI反馈收敛：助手详情内挂载、右上账户入口、独立登录页；MCP/bridge自动管理会话，模拟控件只在默认折叠的原型工具中。中英文、合成数据，不连接生产。

入口：`preview.html`（所有依赖内嵌）；或启动 `python -m http.server 4313 --directory designs/miles-works-domain-v2/prototype`，打开 http://localhost:4313/index.html 。本环境直接file://加载被浏览器管理策略阻止，已通过HTTP验证自包含版本，不冒称双击实测。

- `DESIGN.md`：最新版交互规范和领域边界。
- `FRONTEND-REVIEW.md`：依据AGENTS和Anthropic frontend-design原则逐条审阅。
- `ui-review-validation.json`：本轮24项针对检查，零运行错误。
- `ui-review-offline-validation.json`：自包含HTTP加载、交互、外部请求3项检查及file限制。
- `verify-ui-review.py`：本轮入口与控件分离验证。
- `screenshots/`：最新助手、会话、权限、独立登录和bridge截图。
- `export-preview.py`：重新生成自包含preview.html。
- `_ds/system/`：baoyu-design导入的设计系统副本；`vendor/`为既有固定版本离线依赖。

历史71项矩阵、5项视觉检查和8项解绑检查分别保留在validation.json、polish-validation.json、detachment-validation.json；旧脚本的入口名称可能不适用新版，不能将旧数量算作本轮重新验证。

普通操作只改变内存合成状态，刷新重置；语言偏好保留。额外新建助手仅提供一个样本。全部真实Google、OAuth、设备、会话、bridge与服务端行为仍需工程实施和实机验收。
