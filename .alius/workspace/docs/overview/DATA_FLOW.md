# Alius Core Data Flow

更新时间: 2026-06-05 03:43

## 维护方式

本文件使用 Markdown Mermaid 描述核心数据流。文字流程只能作为说明，不能替代 Mermaid 图。

## 场景一: 记忆存储

```mermaid
flowchart LR
  df_user_input["User Conversation Input"] --> p_cli["Alius CLI including TUI"]
  p_cli --> i_rust["Direct Rust API"]
  i_rust --> df_request["ProtocolEnvelope CoreRequest"]
  df_request --> c_coreapi["Core Public API"]
  c_coreapi --> c_session["Session Manager"]
  c_session --> c_engine["Loop Engine"]
  c_engine --> df_events["CoreEvent Stream"]
  df_events --> m_memory["Memory Manager"]
  m_memory --> mem_router["Memory Classifier"]
  mem_router --> mem_episodic["Episodic Memory"]
  mem_router --> mem_semantic["Semantic Memory"]
  mem_router --> mem_procedural["Procedural Memory"]
  mem_episodic --> store_episodic["episodic.sqlite"]
  mem_semantic --> store_semantic["semantic.sqlite vectors"]
  mem_procedural --> store_procedural["procedural.sqlite"]
```

映射规则:

- session、turn、CoreEvent、工具调用、用户决策写入 episodic memory。
- 用户输入原文是否写入 episodic memory 由 retention / privacy policy 决定，默认可保存摘要或引用。
- 稳定项目事实写入 semantic memory。
- 可复用操作流程写入 procedural memory。

## 场景二: 记忆检索

```mermaid
flowchart LR
  df_task["User Task"] --> c_engine["Loop Engine"]
  c_engine --> m_context["Context Manager"]
  m_context --> m_retrieval["Retrieval Engine"]
  m_retrieval --> r_keyword["Keyword Retrieval"]
  m_retrieval --> r_vector["Vector Retrieval"]
  r_keyword --> r_fusion["Score Fusion"]
  r_vector --> r_fusion
  r_fusion --> r_rerank["Rerank"]
  r_rerank --> r_hits["Top K Memory Hits"]
  r_hits --> m_context
  m_context --> df_model_context["Model Ready Context"]
  df_model_context --> c_engine
```

降级策略:

- semantic vector index 不可用时，降级为 keyword retrieval。
- 单层记忆不可用时，其他记忆层继续工作。

## 场景三: 文档更新

```mermaid
flowchart LR
  df_design_change["User Design Change"] --> p_cli["Alius CLI including TUI"]
  p_cli --> i_protocol["Protocol Interface Layer"]
  i_protocol --> c_coreapi["Core Runtime"]
  c_coreapi --> m_workspace["Workspace Handler"]
  m_workspace --> docs_module["docs modules target document"]
  m_workspace --> docs_history["HISTORY append entry"]
  m_workspace --> m_semantic["Semantic Memory Reindex Request"]
```

约束:

- `SPEC.md` 是需求源头。
- `docs/modules/` 是模块实现标准。
- 所有文档修改必须能追踪到 `HISTORY.md`。

## 场景四: 产品入口到 Core

```mermaid
flowchart TB
  p_cli["Alius CLI including TUI"] --> i_rust["Direct Rust API"]
  p_ide["IDE Extension"] --> i_plugin["Plugin RPC Adapter"]
  p_desktop["Desktop App"] --> i_jsonrpc["JSON RPC Adapter"]
  p_embedded["Embedded SDK"] --> i_ffi["C ABI FFI Adapter"]
  p_third_agent["Third Party Agent App"] --> i_a2a["A2A Protocol Adapter"]

  i_rust --> interface_bus["Interface to Core Entry Bus"]
  i_plugin --> interface_bus
  i_jsonrpc --> interface_bus
  i_ffi --> interface_bus
  interface_bus --> c_coreapi["Core Public API"]
  c_coreapi --> c_session["Session Manager"]
  c_session --> c_engine["Loop Engine"]

  i_ffi --> m_core_lite["Embedded Core Lite"]
  i_a2a --> m_a2a["A2A Adapter"]
  m_a2a --> c_session
```

## 场景五: A2A 通信

```mermaid
sequenceDiagram
  participant p_third_agent as Third Party Agent App
  participant i_a2a as A2A Protocol Adapter
  participant m_a2a as A2A Adapter
  participant m_soul as Soul Manager
  participant c_session as Session Manager
  participant c_engine as Loop Engine

  p_third_agent->>i_a2a: A2A task
  i_a2a->>m_a2a: normalized A2A message
  m_a2a->>m_soul: load Agent Card skills policy
  m_a2a->>c_session: map to CoreRequest
  c_session->>c_engine: start run loop
  c_engine-->>m_a2a: CoreEvent stream
  m_a2a-->>p_third_agent: A2A response
```

启用策略:

- CLI: `--a2a` / config 可选启用。
- Desktop: settings toggle，规划可选启用。
- 第三方 Agent: A2A 标准入口。
- Embedded SDK: 默认关闭。

## 场景六: 模型调用

```mermaid
sequenceDiagram
  participant c_engine as Loop Engine
  participant m_budget as Budget Manager
  participant m_model as Model Router
  participant m_provider as Provider Manager
  participant x_model as Model Provider APIs

  c_engine->>m_model: ModelRequest
  m_model->>m_budget: check cost and token budget
  m_budget-->>m_model: BudgetDecision
  m_model->>m_provider: ProviderRequest
  m_provider->>x_model: provider API call
  x_model-->>m_provider: provider stream
  m_provider-->>c_engine: normalized ProviderEvent stream
```

## 场景七: 工具调用

```mermaid
sequenceDiagram
  participant c_engine as Loop Engine
  participant m_tool as Tool Executor
  participant m_shell_gate as Shell Gate
  participant m_security as Security And Policy Manager
  participant x_local as Local OS Capabilities
  participant x_mcp as External MCP Servers
  participant m_budget as Budget Manager
  participant m_logging as Logging Manager
  participant m_store as Storage Manager

  c_engine->>m_tool: ToolCallRequest
  m_tool->>m_security: authorize capability
  m_security-->>m_tool: allow deny or approval required
  alt shell process or git tool
    m_tool->>m_shell_gate: inspect command args cwd scope
    m_shell_gate->>m_security: authorize shell scope
    m_security-->>m_shell_gate: allow deny or approval required
    m_shell_gate->>m_logging: audit decision
    m_shell_gate->>x_local: authorized local execution
  else local non-shell tool
    m_security->>x_local: authorized file capability
  else mcp tool
    m_tool->>x_mcp: MCP tool call
  end
  m_tool->>m_budget: usage and failure update
  m_tool->>m_logging: runtime or error log
  m_tool->>m_store: persist trace
  m_tool-->>c_engine: ToolEvent stream
```

工具范围:

- Read / Edit / Write / Grep。
- Bash / WebFetch / WebSearch。
- AskUserQuestion / Plan / Todo。
- Agent / Task / MCP Resource。

## 场景八: Shell 门禁

```mermaid
flowchart TB
  sh_req["Shell ToolCallRequest"] --> sh_parse["Parse command and args"]
  sh_parse --> sh_cwd["Resolve cwd and workspace root"]
  sh_cwd --> sh_scope["Detect read write delete scope"]
  sh_scope --> sh_risk["Classify risk"]
  sh_risk --> sh_policy["Security Policy decision"]
  sh_policy --> sh_deny["Deny"]
  sh_policy --> sh_approval["Approval Required"]
  sh_policy --> sh_allow["Allow"]
  sh_deny --> sh_audit["Audit Log"]
  sh_approval --> sh_audit
  sh_allow --> sh_exec["Execute within authorized scope"]
  sh_exec --> sh_runtime_log["Runtime or Error Log"]
```

规则:

- `rm -rf` 类 critical destructive 命令默认拒绝或强制审批。
- cwd、参数、glob、symlink、重定向和 `bash -c` 都必须纳入作用范围分析。
- 读写 workspace 外路径必须授权。
- 无法确定作用范围时，不自动允许。

## 场景九: 运行日志

```mermaid
flowchart LR
  log_session["Session Manager"] --> log_ctx["workspace session run trace metadata"]
  log_engine["Loop Engine"] --> log_mgr["Logging Manager"]
  log_provider["Provider Manager"] --> log_mgr
  log_tool["Tool Executor"] --> log_mgr
  log_shell["Shell Gate"] --> log_mgr
  log_policy["Security Policy"] --> log_mgr
  log_ctx --> log_mgr
  log_mgr --> log_runtime["runtime.log.jsonl"]
  log_mgr --> log_error["error.log.jsonl"]
  log_mgr --> log_audit["audit.log.jsonl"]
  log_mgr --> log_event["CoreEvent LogRecordEmitted"]
```

要求:

- runtime、error、exception、audit 日志实时记录。
- error、exception、audit 必须立即 flush。
- 日志必须脱敏 API key、token、Authorization header 和敏感用户输入。

## 场景十: 上下文压缩

```mermaid
flowchart LR
  c_engine["Loop Engine"] --> m_context["Context Manager"]
  m_context --> ctx_window["Track Context Window"]
  ctx_window --> ctx_reserve["Reserve Compression Space"]
  ctx_reserve --> m_compress["Compression Worker"]
  m_compress --> compress_budget["Reserve 20000 Tokens"]
  compress_budget --> compress_summary["Compressed Summary With Source Map"]
  compress_summary --> mem_semantic["Semantic Memory"]
  compress_summary --> mem_episodic["Episodic Memory"]
  compress_summary --> m_context
```

## 场景十一: 预算和熔断

```mermaid
flowchart TB
  c_engine["Loop Engine"] --> m_budget["Budget Manager"]
  m_budget --> budget_token["Check Token Budget"]
  m_budget --> budget_time["Check Time Budget"]
  m_budget --> budget_cost["Check Cost Budget"]
  m_budget --> budget_tool["Check Tool Call Budget"]
  m_budget --> budget_failure["Record Failure Count"]
  budget_token --> budget_terminate["Terminate If Exceeded"]
  budget_failure --> budget_break["Circuit Break If Failures >= 3"]
  budget_terminate --> budget_event["BudgetDecision Event"]
  budget_break --> budget_event
```

## 场景十二: 构建与 Feature 裁剪

```mermaid
flowchart TB
  b_policy["Feature Policy"] --> b_cli["CLI Build"]
  b_cli --> cli_command["cargo build --release"]
  b_cli --> cli_features["features cli full default"]
  b_cli --> core_full["Complete Core Runtime"]

  b_policy --> b_embedded["Embedded SDK Build"]
  b_embedded --> emb_command["cargo build --release --features embedded-sdk"]
  b_embedded --> emb_crate["crate type staticlib cdylib"]
  b_embedded --> m_core_lite["FFI plus Core Lite"]
  m_core_lite -. disables .-> emb_disabled["heavy tools LanceDB local embedding plugin runtime"]
```
