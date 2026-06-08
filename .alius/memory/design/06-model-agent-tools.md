# 06. 模型、代理与工具

更新时间: 2026-06-04 22:10

## Provider 抽象

`alius-model` 定义统一 `LlmProvider`:

```rust
chat_stream(conversation)
chat_once(prompt, system)
list_models()
chat_stream_with_tools(conversation, tools)
continue_with_tool_results(conversation, tool_results, tools)
```

`LlmClient` 根据 `ProviderType` 分派:

| ProviderType | 当前实现 |
| --- | --- |
| `Openai` | OpenAI-compatible provider |
| `BigModel` | 走 OpenAI-compatible provider |
| `Custom` | 走 OpenAI-compatible provider |
| `Anthropic` | Anthropic native provider |
| `Google` | 返回未实现错误 |

## OpenAI-compatible provider

使用 `async-openai`:

- streaming chat 使用 `create_stream()`。
- `chat_once()` 使用非 streaming completion。
- `list_models()` 调 OpenAI models list。
- tool calling 使用非 streaming request。
- Summary message 映射为 system message。

适用:

- OpenAI。
- BigModel GLM OpenAI 兼容接口。
- 自定义 OpenAI-compatible endpoint。

## Anthropic provider

使用 `reqwest` 调 Anthropic Messages API:

- streaming 使用 SSE 解析。
- system prompt 使用顶层 `system` 字段。
- tool calling 使用 Anthropic tool schema。
- tool result 使用 user message 的 `tool_result` content block。
- `list_models()` 调 `<base_url>/models`。

## Conversation

`Conversation` 是内存态上下文:

- messages
- system_prompt
- max_tokens
- summary

当前能力:

- 添加 user/assistant message。
- 加载已有 messages。
- 粗略估算是否需要 summarization。
- 插入 summary message。

当前限制:

- summarization 逻辑没有自动接入主聊天路径。
- max_tokens 默认 4096，仅用于本地估算。

## Agent loop

`AliusAgent::handle_message()` 设计为完整工具调用循环:

1. 添加 user message。
2. 将 tool definitions 发给模型。
3. 模型返回文本和可选 tool calls。
4. 对每个 tool call 发事件。
5. 如工具要求确认:
   - auto_confirm 开启时自动确认。
   - auto_confirm 关闭时当前默认拒绝，并把拒绝作为 tool result 回填。
6. 执行工具。
7. 将 tool results 回填给模型。
8. 最多允许 10 次 tool calls。
9. 无 tool call 后结束 turn。

当前接线状态:

- `ReplSession::new()` 已创建 `AliusAgent`。
- 但 workspace 和旧 REPL 的普通聊天路径当前直接调用 `client.chat_stream()`。
- 所以默认交互不会让 LLM 实际调用内置工具。

后续接线建议:

- workspace 执行路径从 `collect_model_response()` 改为消费 `AliusAgent` 事件流。
- 将 tool confirmation 映射为 `InteractionUi::Decision`。
- 将 tool events 映射为 ConversationBlock 和 Plan evidence。

## 工具体系

核心 trait:

```rust
AliusTool {
  name()
  description()
  input_schema()
  required_permission()
  requires_confirmation(args)
  confirmation_request(args)
  execute(args, ctx)
}
```

`ToolRegistry`:

- 注册工具。
- 按名称查找工具。
- 导出 OpenAI tool JSON。
- 导出 provider-agnostic `ToolDef`。

`ToolContext`:

- workspace
- session_id
- working_directory

## 内置工具

| 工具 | 能力 | 当前安全边界 |
| --- | --- | --- |
| `read_file` | 读取文件 | canonicalize 后要求在 workspace 内 |
| `list_dir` | 列目录 | canonicalize 后要求在 workspace 内 |
| `write_file` | 写文件 | parent 检查 workspace，要求确认 |
| `edit_file` | 精确字符串替换 | canonicalize 后要求在 workspace 内，要求确认 |
| `search` | grep 搜索 | canonicalize 后要求在 workspace 内，最多 50 行 |
| `find_files` | find 文件 | canonicalize 后要求在 workspace 内 |
| `move_file` | 移动或重命名 | source 检查 workspace，要求确认 |
| `delete_file` | 删除文件或空目录 | canonicalize 后要求在 workspace 内，要求确认 |
| `create_dir` | 创建目录 | parent 检查 workspace，要求确认 |
| `git_status` | git status short + branch | read |
| `git_diff` | git diff | read，超长截断 |
| `http_request` | HTTP GET/POST/PUT/DELETE | 非 GET 要求确认 |
| `code_stats` | 文件数和行数统计 | workspace 内 |
| `todo` | session 内内存 todo | 进程内 HashMap |
| `json` | JSON parse/get/keys/values/validate | 本地纯解析 |
| `shell` | 执行 shell 命令 | 当前只做少量危险 pattern 拦截 |

## 权限结构

`PermissionLevel`:

- Read
- Write
- Execute
- Admin

`PermissionManager`:

- enabled levels
- allowed tools
- denied tools
- require confirmation

当前限制:

- `PermissionManager` 未在 `AliusAgent::execute_tool()` 中强制检查。
- 部分工具没有覆盖 `required_permission()`，例如 `shell` trait 默认是 Read，但 `PermissionManager::level_for_tool()` 中将 shell 视为 Execute。
- 当前实际生效更多依赖每个 tool 的 `requires_confirmation()`。

## 当前风险点

1. 默认交互不走 agent loop，工具能力未进入主体验。
2. tool permission manager 未集中强制执行。
3. `ConversationStore::append_message()` 使用 `std::fs::write`，当前会覆盖文件，不是真正 append；主路径用 `save_messages()`，但该方法名和注释不一致。
4. `move_file` 对 destination 缺少和 source 对称的 workspace containment 校验。
5. `shell` 的安全拦截很基础，不适合在无确认或无权限策略下暴露给 LLM。
6. HTTP 工具没有域名白名单或网络策略。

## 设计方向

短期:

- 先把 agent event 映射进 workspace UI。
- 写入工具调用的 evidence。
- 接入确认 decision。
- 强制 PermissionManager。

中期:

- 为 project `.alius/config.toml` 增加工具权限配置。
- 将工具调用结果纳入 session messages 或独立 trace。
- 明确 shell/http 的默认禁用策略。
