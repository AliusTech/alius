# Mermaid Diagram Catalog

更新时间: 2026-06-05 03:43

## 定位

本文件是 workspace 中架构图、流程图和构建关系图的 Markdown Mermaid 源文件。后续不再以外部绘图文件或图片文件作为设计图主来源。

## Mermaid ID 规范

- Product Layer 节点使用 `p_*`。
- Protocol Interface Layer 节点使用 `i_*`。
- Core Runtime 节点使用 `c_*`、`m_*`、`d_*`。
- External Resources 节点使用 `x_*`。
- Build / Feature 节点使用 `b_*`。
- 数据流节点使用 `df_*`。
- 实体关系节点使用大写实体名，详见 `ENTITY_RELATIONSHIP.md`。

## Overall Architecture

```mermaid
flowchart TB
  subgraph product_layer["Product Layer"]
    p_cli["Alius CLI including TUI"]
    p_ide["IDE Extension VS Code JetBrains"]
    p_embedded["Embedded Third Party SDK ESP32 LVGL"]
    p_desktop["Desktop App Electron planned"]
    p_third_agent["Third Party Agent App"]
  end

  subgraph interface_layer["Protocol Interface Layer"]
    i_rust["Direct Rust API"]
    i_plugin["Plugin RPC Adapter JSON RPC LSP like stdio socket"]
    i_ffi["C ABI FFI Adapter"]
    i_jsonrpc["JSON RPC Adapter stdio socket"]
    i_a2a["A2A Protocol Adapter"]
    interface_bus["Interface to Core Entry Bus"]
  end

  subgraph core_layer["Core Runtime"]
    c_coreapi["Core Public API"]
    c_session["Session Manager"]
    c_engine["Loop Engine"]
    m_prompt["Prompt Builder"]
    m_soul["Soul Manager Agent Card Source"]
    m_memory["Memory System"]
    m_a2a["A2A Adapter"]
    m_model["Model Router"]
    m_provider["Provider Manager"]
    m_tool["Tool Executor"]
    m_shell_gate["Shell Gate"]
    m_workflow["Workflow Engine"]
    m_store["Storage Manager"]
    m_security["Security and Policy Manager"]
    m_logging["Logging Manager"]
    m_budget["Budget Manager"]
    m_context["Context Manager"]
    m_compress["Compression Worker"]
    m_core_lite["Embedded Core Lite"]
    m_a2a_switch["A2A Enable Switch"]
    d_soul["Installed Soul Data"]
    d_memory["Memory Data"]
    d_logs["Runtime Logs Audit Logs"]
  end

  subgraph external_group["External Resources"]
    x_model["Model Provider APIs"]
    x_embed["Embedding APIs Memory Gateway"]
    x_local["Local OS Capabilities"]
    x_mcp["External MCP Servers"]
    x_a2a["Remote A2A Agents Apps"]
    x_soulrepo["Official Soul Repository"]
  end

  subgraph build_group["Build Targets And Cargo Features"]
    b_cli["CLI Build cli full default"]
    b_embedded["Embedded SDK Build embedded sdk"]
    b_policy["Feature Policy"]
  end

  p_cli --> i_rust
  p_ide --> i_plugin
  p_embedded --> i_ffi
  p_desktop --> i_jsonrpc
  p_third_agent --> i_a2a
  p_cli -. optional enable .-> i_a2a
  p_desktop -. planned optional .-> i_a2a

  i_rust --> interface_bus
  i_plugin --> interface_bus
  i_ffi --> interface_bus
  i_jsonrpc --> interface_bus
  interface_bus --> c_coreapi
  i_a2a --> m_a2a

  c_coreapi --> c_session
  c_session --> c_engine
  c_session --> m_soul
  c_session --> m_memory
  c_engine --> m_model
  m_model --> m_provider
  c_engine --> m_tool
  m_tool --> m_shell_gate
  c_engine --> m_workflow
  c_engine --> m_store
  c_engine --> m_security
  c_engine --> m_logging
  c_engine --> m_budget
  c_engine --> m_context
  m_context --> m_compress
  m_soul --> m_prompt
  m_prompt --> c_engine
  m_soul --> m_a2a
  m_a2a_switch --> m_a2a
  m_a2a --> c_session
  i_ffi --> m_core_lite
  m_core_lite --> m_model
  m_core_lite --> m_memory
  m_core_lite --> m_store
  m_core_lite --> m_security
  m_soul --> d_soul
  m_memory --> d_memory
  m_logging --> d_logs
  c_session --> m_logging
  m_provider --> m_logging
  m_tool --> m_logging
  m_shell_gate --> m_security
  m_shell_gate --> m_logging
  m_security --> m_logging
  m_memory --> c_engine

  m_provider --> x_model
  m_memory --> x_embed
  m_security --> x_local
  m_tool --> x_mcp
  m_a2a --> x_a2a
  p_third_agent --> x_a2a
  d_soul --> x_soulrepo
  m_core_lite --> x_model
  m_core_lite --> x_embed

  b_policy --> b_cli
  b_policy --> b_embedded
  b_cli --> p_cli
  b_embedded --> p_embedded
  b_embedded --> i_ffi
  b_embedded --> m_core_lite
```

## Protocol To Core

```mermaid
flowchart LR
  p_any["Product Entry"] --> i_transport["Transport Adapter"]
  i_transport --> i_envelope["ProtocolEnvelope"]
  i_envelope --> i_origin["Origin Capability Normalizer"]
  i_origin --> interface_bus["Interface to Core Entry Bus"]
  interface_bus --> c_coreapi["Core Public API"]
  c_coreapi --> c_session["Session Manager"]
  c_session --> c_engine["Loop Engine"]
```

## Core Runtime Loop

```mermaid
flowchart TB
  c_session["Session Manager"] --> c_engine["Loop Engine"]
  c_engine --> m_soul["Soul Manager"]
  m_soul --> m_prompt["Prompt Builder"]
  c_engine --> m_context["Context Manager"]
  m_context --> m_memory["Memory System"]
  m_context --> m_compress["Compression Worker"]
  c_engine --> m_model["Model Router"]
  m_model --> m_provider["Provider Manager"]
  c_engine --> m_tool["Tool Executor"]
  m_tool --> m_shell_gate["Shell Gate"]
  m_shell_gate --> m_security["Security and Policy Manager"]
  c_engine --> m_budget["Budget Manager"]
  c_engine --> m_store["Storage Manager"]
  c_engine --> m_logging["Logging Manager"]
  m_provider --> m_logging
  m_tool --> m_logging
  m_security --> m_logging
  c_engine --> c_event["CoreEvent Stream"]
```

## Shell Command Gate Flow

```mermaid
sequenceDiagram
  participant c_engine as Loop Engine
  participant m_tool as Tool Executor
  participant m_shell_gate as Shell Gate
  participant m_security as Security Policy
  participant x_local as Local OS Capabilities
  participant m_logging as Logging Manager

  c_engine->>m_tool: shell/process/git ToolCallRequest
  m_tool->>m_shell_gate: inspect command args cwd scope
  m_shell_gate->>m_security: request policy decision
  alt denied
    m_shell_gate->>m_logging: audit deny
    m_shell_gate-->>m_tool: Deny
  else approval required
    m_security-->>m_tool: ApprovalRequired
    m_tool-->>c_engine: ToolApprovalRequired event
  else allowed
    m_shell_gate->>x_local: execute within authorized scope
    x_local-->>m_tool: output status
    m_tool->>m_logging: runtime/error log
  end
```

## Runtime Logging Flow

```mermaid
flowchart LR
  c_session["Session Manager"] --> log_ctx["Log Context workspace session run trace"]
  c_engine["Loop Engine"] --> m_logging["Logging Manager"]
  m_provider["Provider Manager"] --> m_logging
  m_tool["Tool Executor"] --> m_logging
  m_shell_gate["Shell Gate"] --> m_logging
  m_security["Security and Policy Manager"] --> m_logging
  log_ctx --> m_logging
  m_logging --> log_runtime["runtime.log.jsonl"]
  m_logging --> log_error["error.log.jsonl"]
  m_logging --> log_audit["audit.log.jsonl"]
  m_logging --> log_stream["Product Visible Log Events"]
```

## A2A Flow

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
  m_a2a->>c_session: map task to CoreRequest
  c_session->>c_engine: start run loop
  c_engine-->>m_a2a: CoreEvent stream
  m_a2a-->>p_third_agent: A2A response events
```

## Memory Retrieval Flow

```mermaid
flowchart LR
  df_query["User Task Query"] --> m_context["Context Manager"]
  m_context --> m_retrieval["Retrieval Engine"]
  m_retrieval --> m_keyword["Keyword Retrieval"]
  m_retrieval --> m_vector["Vector Retrieval"]
  m_keyword --> m_fusion["Score Fusion"]
  m_vector --> m_fusion
  m_fusion --> m_rerank["Rerank"]
  m_rerank --> m_hits["Top K Memory Hits"]
  m_hits --> m_context
  m_context --> c_engine["Loop Engine"]
```

## Build And Feature Policy

```mermaid
flowchart TB
  b_policy["Feature Policy"] --> b_cli["CLI Build cli full default"]
  b_policy --> b_embedded["Embedded SDK Build embedded sdk"]
  b_cli --> p_cli["Alius CLI including TUI"]
  b_cli --> core_layer["Complete Core Runtime"]
  b_embedded --> p_embedded["Embedded Third Party SDK"]
  b_embedded --> i_ffi["C ABI FFI Adapter"]
  b_embedded --> m_core_lite["Embedded Core Lite"]
  m_core_lite -. disables .-> disabled["heavy tools LanceDB local embedding plugin runtime"]
```

## Workspace Document Confirmation

```mermaid
flowchart LR
  ws_work["workspace working version"] --> ws_diff["Directory Compare"]
  ws_archive["workspace archive completed version"] --> ws_diff
  ws_diff --> ws_added["Added Files"]
  ws_diff --> ws_deleted["Deleted Files"]
  ws_diff --> ws_modified["Modified Files"]
  ws_diff --> ws_confirm["User Confirmation"]
  ws_confirm --> ws_update["Overwrite archive snapshot"]
  ws_update --> ws_history["Append HISTORY"]
```
