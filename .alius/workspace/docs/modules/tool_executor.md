# Tool Executor

更新时间: 2026-06-05 03:43

## 模块职责

Tool Executor 负责统一执行文件、环境、会话、协作、MCP 和 Agent/Task 工具。

图中工具范围:

- Read / Edit / Write / Grep
- Bash / WebFetch / WebSearch
- AskUserQuestion / Plan / Todo
- Agent / Task / MCP Resource

输入:

- `ToolCallRequest`
- `CapabilityScope`
- origin metadata
- approval result
- shell gate decision

输出:

- `ToolCallStarted`
- `ToolCallDelta`
- `ToolCallCompleted`
- `ToolCallFailed`

## 接口定义

```text
execute_tool(request: ToolCallRequest) -> Stream<ToolEvent>
```

```text
register_tool(definition: ToolDefinition) -> Result<ToolRef>
```

```text
list_tools(scope: CapabilityScope) -> Result<Vec<ToolDefinition>>
```

异常:

- 工具不存在。
- 权限不足。
- 审批被拒绝。
- 工具执行超时或失败。

## 内部逻辑

```text
receive tool call from Loop Engine
-> resolve tool definition
-> request Security & Policy check
-> if shell/process/git, request Shell Gate inspection
-> request user approval if required
-> execute tool adapter
-> stream result
-> persist trace
-> emit runtime logs
-> update budget
```

## 数据存储

读取:

- `.alius/config/tools.toml`
- `.alius/config/permissions.toml`
- `.alius/config/mcp.json`

写入:

- `.alius/memory/episodic/` 工具事件。
- Storage Manager trace。
- `.alius/memory/logs/` 运行日志和错误日志。

## 外部资源

```text
Tool Executor -> Shell Gate -> Security Policy -> Local OS Capabilities
Tool Executor -> External MCP Servers
```

## 异常处理

- 本地文件、shell、git 工具必须先经过 sandbox 和 allowlist。
- shell、process、git 工具必须先经过 Shell Gate；命令、参数和作用范围不能超过 workspace，除非获得授权。
- MCP server 不可用时只禁用对应工具，不影响非 MCP 工具。
- 工具连续失败计入 Budget Manager 熔断计数。
- Tool adapter 执行失败必须记录 error log。

## 与其他模块的关系

- 由 Loop Engine 调用。
- 受 Security & Policy Manager 控制。
- shell/process/git 受 Shell Gate 控制。
- usage 和失败上报 Budget Manager。
- trace 写入 Storage Manager。
- runtime、error、audit 事件写入 Logging Manager。
- MCP 工具读取 Config Manager 输出。

## 验收标准

- 所有工具调用都经 Tool Executor。
- 工具调用前必有 policy 决策。
- shell/process/git 调用前必有 Shell Gate 决策。
- 工具结果能进入 CoreEvent stream。
- MCP Resource 不绕过工具权限模型。
