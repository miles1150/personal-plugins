# Codex CU MCP bridge

将本机已安装的 OpenAI Chrome Browser Use 和桌面 Computer Use 运行时接入 Claude Code 等 stdio MCP 客户端。统一 MCP 名称为 `codex-cu`。需要 macOS、Python 3 和已安装的 ChatGPT/Codex 桌面运行时；Chrome 路径另需已连接的 OpenAI Chrome 扩展和 Native Messaging Host，桌面路径需要原生 Computer Use 服务及其系统权限。

这是非官方适配器，不分发或修改 OpenAI 的程序，不移除权限检查，不自动同意确认请求。独立于当前 Codex 聊天；不承诺脱离已安装的桌面运行环境。退出 ChatGPT/Codex App 后能否继续使用尚未验证。`codex-chrome` 是旧的浏览器专用注册名；本机迁移后只保留 `codex-cu`。

## 已验证

2026-10-24，本机 Claude Code 2.1.296，OpenAI `@oai/browser-desktop` 0.1.1，`unified-computer-use` 26.1013.40934：

- 独立 Python MCP 客户端使用自己生成的 session/turn ID，完成 Chrome 打开网页、点击链接、读取跳转结果和截图。
- `claude -p` 只加载本适配器，完成相同流程：`example.com` → 点击 Learn more → `https://www.iana.org/help/example-domains`；读取标题 `Example Domains`、获取截图、成功执行 `finish_turn`。
- 该轮 Claude 使用本机默认模型；实际初始化报告 `claude-opus-5-5[1m]`。未指定或替换模型。
- 断开 MCP 的清理请求成功返回。用户原有标签页没有被认领或操作。

上述 Chrome 验证来自 0.1 版本；合并版本的桌面授权、计算器和 Chrome 回归结果见 `artifacts/combined-validation/verification.json`（本机生成，不进入 Git）。

0.2 合并版本的实际 Claude Code 验收：

- 负向分支：1 次真实 `elicitation/create` 被 session-local hook 拒绝，服务端返回 `Computer Use was not approved to use Calculator`，未返回计算器界面；随后清理成功。
- 正向分支：15 次 Calculator 空表单授权请求经 Claude Code 接受并转回原始服务；实际清空计算器后点击 `1、2、乘、1、2、等于`，读取到 `12×12` 和 `144` 并截图。原始服务工具执行错误为 0。
- 同一 MCP/REPL 中的 Chrome 打开、点击、页面读取和截图回归成功。
- 两次显式 `finish_turn` 均成功；第一次后使用新的 turn ID 再次授权读取计算器，证明轮次可以继续使用。退出时的额外清理也成功。
- 单元测试 11 项通过，涵盖双向 ID 冲突、授权答复转发、拒绝用例、两类服务注册、清理失败不换轮次，以及 JS 的非只读标注。
- 本机用户注册已更名为 `codex-cu`，旧的 `codex-chrome` 注册已移除。正常注册不包含测试 hook 或 trace 环境变量。

仅代表记录中的流程通过，不代表所有网站、应用、登录、文件传输、企业策略或 App 退出后的行为都已验收。

## 为什么默认一个 MCP

Chrome 和桌面共用同一 Node REPL、连接级 session ID、任务级 turn ID 和 `finish_turn`。一个 MCP 可以在同一任务里保留浏览器与应用对象，统一转发授权并清理，且只暴露 `js`、`js_reset`、`finish_turn` 三个工具。

不同的应用访问仍由原始服务逐项授权。单 MCP 不意味着把 Chrome 许可扩展为所有桌面应用许可。若需要按客户端部署范围隔离，或希望一个进程失败不影响另一类能力，可分别用 `--surfaces browser`、`--surfaces computer` 启动两个实例；仅改服务器名称不能形成能力隔离。

## 安装到 Claude Code

在本目录运行（注册为当前用户的 `codex-cu`，不会覆盖其他名字的服务器）：

```bash
claude mcp add codex-cu --scope user -- python3 "$PWD/server.py"
```

其他位置使用 server.py 的绝对路径。注册后新启动 Claude Code，通过 `/mcp` 查看连接与工具。

也可只对一次运行加载配置：

```json
{
  "mcpServers": {
    "codex-cu": {
      "type": "stdio",
      "command": "python3",
      "args": ["/absolute/path/to/server.py"]
    }
  }
}
```

配合 `claude -p --strict-mcp-config --mcp-config /path/to/config.json`。非交互调用仍需要按任务明确允许对应工具；不要通过跳过全部权限检查来解决确认失败。

## 工具与使用

- `js`：持久化 JavaScript REPL，调用原始 `cua` 浏览器和桌面 API。标注 `readOnlyHint: false`，避免客户端将界面操作当作只读调用并行执行。
- `js_reset`：清空 REPL 状态。
- `finish_turn`：调用原始 `turn_ended` 清理当前任务，并为下一任务生成新的 turn ID。临时标签页可能关闭；先获取所需结果。需要保留成果页时，按返回的原始文档使用 `markDeliverable()`。

首次调用 js 时只执行一个与任务对应的入口。例如 Chrome：

```javascript
var browser = await cua.getBrowser({ id: "chrome" });
```

阅读返回的文档后再创建标签页：

```javascript
var tab = await cua.createBrowserTab("chrome", "https://example.com", {
  sessionName: "🔎 Chrome task"
});
```

根据最新界面决定点击目标；操作后读取 `getAXState()`。结束时调用 `finish_turn`。首次文档较长，部分客户端会把工具结果存为文件；客户端应允许模型读取该结果文档，或按实际显示的说明与 API 工作。

桌面任务的首次入口：

```javascript
var calculator = await cua.getApp("com.apple.calculator");
```

原生服务会经 MCP `elicitation/create` 请求“Allow Computer Use to use Calculator?”。适配层原样转交 Claude Code，再将用户的接受、拒绝或取消答复原样转回。没有接受时不应继续读取或操作该应用。

此版本未实现授权缓存，也未将 Codex 自定义的 `persist` 元数据转换成 Claude 的持久授权 UI。因此一个桌面任务可能对同一应用产生多次确认；本次计算器验收实际发生 15 次往返。测试 hook 处理了这次已授权的确认，正常使用不会自动接受。

桌面与 Chrome 可以在同一个 REPL/任务中使用；首次创建另一类对象时同样阅读其返回文档。`finish_turn` 结束厂商运行时的当前轮次，不会清空 JavaScript 变量或退出用户应用。

### 无交互授权验收

`verification/calculator_approval.py` 是测试夹具，不会随 MCP 注册或写入用户 hooks。它只能通过本次 `claude --settings /path/to/test-settings.json` 进程配置启用：仅匹配服务器 `codex-cu`、Calculator 的精确提示和空 form schema，其他请求全部拒绝。`--deny` 用于负向验证。

因此测试可以基于用户明确授予的计算器权限无交互运行，正常使用依然显示 Claude Code 的确认界面。不要把此夹具安装为通用自动授权策略。

## 宿主依赖的实际含义

运行时从 MCP `tools/call` 的 `params._meta["x-codex-turn-metadata"]` 读取字符串 `session_id` 和 `turn_id`。适配器为每条连接生成独立 UUID；同一任务的多个 js 调用使用同一 turn ID。不是借用现有 Codex 聊天或伪装其身份。

已检查的实现：

1. 浏览器发现首先验证两个字段存在。
2. Chrome 扩展默认 `discoveryScope: global`；IAB/MCP Apps 默认 `session`，还需匹配宿主会话，因此它们不能用此结论类推。
3. 后续请求携带 `session_id`、`turn_id`、`session_context: live`，用于标签页归属、策略上下文和 turn-ended 清理。
4. 默认配置 `CUA_REPL_ENABLED_SURFACES=browser,computer`、`BROWSER_USE_AVAILABLE_BACKENDS=chrome`，可信服务注册 `browser → @oai/browser-desktop/service` 和 `sky → @oai/sky/service`。`--surfaces browser` 或 `--surfaces computer` 会同时缩小 surface 和可信服务注册范围。
5. 原始服务端的 elicitation 请求及客户端答复双向转发。适配器没有自动批准逻辑，也不设置任何关闭安全检查的标志。
6. `finish_turn` 成功后才更换 turn ID；客户端断开或进程收到 SIGTERM/SIGINT 时，也会尝试结束当前轮次。

连接级 ID 不等于 Claude 用户消息的 ID。此版本以显式 `finish_turn` 划分任务；尚未接入 Claude Stop hooks。异常硬终止无法保证清理。

## 本地诊断

自动寻找 `~/.codex/plugins/cache/openai-bundled/unified-computer-use/*/.mcp.json` 中最新可用入口；支持 `CODEX_HOME` 或 `--manifest PATH`。更新后程序路径、私有元数据或协议可能改变。

```bash
python3 -m unittest discover -s tools/codex-cu-mcp -p 'test_*.py'
```

可选环境变量 `CODEX_CU_MCP_TRACE=/absolute/path/wire.jsonl` 记录四个方向的 JSON-RPC 消息（旧的 `CODEX_CHROME_MCP_TRACE` 仍可兼容）。日志可能包含页面内容、截图和确认信息，只用于本机诊断；默认不记录。`artifacts/` 和 JSONL 已在本目录忽略。

移除注册：

```bash
claude mcp remove codex-cu --scope user
```
