# 未实现设计清单

更新时间: 2026-06-05 23:00

本文列出所有已在 `docs/modules/` 中设计但尚未在代码中实现的功能。每项包含设计文档、代码位置和当前状态。

---

## A. Loop Engine — Plan 模式

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| planner.rs | `docs/modules/workflow_engine.md` | `runtime/core/src/loop_engine/planner.rs` | **已实现** — Plan/PlanStep/PlanStatus 结构体及方法 |
| tool_step.rs | `docs/modules/tool_executor.md` | `runtime/core/src/loop_engine/tool_step.rs` | **已实现** — execute_tools() 通过 ToolRegistry 调度 |
| Plan 模式执行 | `docs/modules/workflow_engine.md` | `runtime/core/src/loop_engine/engine.rs` Plan 分支 | **已实现** — run_plan() 多迭代循环 + 工具执行 + 收敛检查 |
| Plan 完整流程 | `docs/modules/workflow_engine.md` | `runtime/core/src/loop_engine/engine.rs` | **已实现** — planning step → tool step → convergence 完整链路 |

---

## B. Provider — Google AI

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| Google Generative AI | `docs/modules/provider_manager.md` | `runtime/model/src/client.rs:70` | 返回 "not yet implemented" |

---

## C. 三层记忆系统

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| Episodic Memory | `docs/modules/memory_manager/episodic_memory.md` | — | 未实现 |
| Semantic Memory | `docs/modules/memory_manager/semantic_memory.md` | — | 未实现 |
| Procedural Memory | `docs/modules/memory_manager/procedural_memory.md` | — | 未实现 |
| Retrieval Engine | `docs/modules/retrieval_engine.md` | — | 未实现 |
| 自动分类 | `docs/modules/memory_manager/README.md` classify_memory | — | 未实现 |
| Retention Policy | `docs/modules/memory_manager/README.md` apply_retention_policy | — | 未实现 |

当前实现：`runtime/store/src/memory.rs` 为 flat JSON 存储（global + project 两级），无 SQLite、无 embedding 检索、无自动分类。

---

## D. Logging Manager

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| 结构化日志写入 | `docs/modules/logging_manager.md` emit | — | 未实现 |
| JSONL 文件持久化 | `docs/modules/logging_manager.md` 数据存储 | — | 未实现 |
| 日志轮转 | `docs/modules/logging_manager.md` rotate | — | 未实现 |
| 脱敏规则 | `docs/modules/logging_manager.md` 脱敏规则 | — | 未实现 |
| 实时流订阅 | `docs/modules/logging_manager.md` stream | — | 未实现 |
| flush | `docs/modules/logging_manager.md` flush | — | 未实现 |

当前实现：`CoreRuntime::query_logs()` 基于 `SessionManager` 内存事件查询，无文件持久化。

---

## E. Model Router

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| 三级路由（light/medium/high） | `docs/modules/model_router.md` route_model | — | 未实现 |
| 成本估算 | `docs/modules/model_router.md` estimate_route_cost | — | 未实现 |
| Fallback 路由 | `docs/modules/model_router.md` fallback_route | — | 未实现 |

当前实现：单一 model 设置，无路由逻辑。

---

## F. Prompt Builder

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| L1-L5 分层 Prompt | `docs/modules/prompt_builder.md` build_prompt | — | 未实现 |
| Budget 校验 | `docs/modules/prompt_builder.md` validate_prompt_budget | — | 未实现 |

当前实现：system prompt 从 `runtime_config::system_prompt_for_role()` 生成，无分层构建。

---

## G. Budget Manager

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| Token/Time/Cost 预算 | `docs/modules/budget_manager.md` check_budget | — | 未实现 |
| 连续失败熔断 | `docs/modules/budget_manager.md` record_failure | — | 未实现 |
| Token 预留 | `docs/modules/budget_manager.md` reserve_tokens | — | 未实现 |

---

## H. Context Manager

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| 上下文构建 | `docs/modules/context_manager.md` build_context | — | 未实现 |
| 截断策略 | `docs/modules/context_manager.md` truncate_context | `runtime/core/src/loop_engine/context.rs` | **部分实现** — tiktoken 估算 + needs_truncation() + truncate() |
| 压缩请求 | `docs/modules/context_manager.md` request_compression | — | 未实现 |

当前实现：`runtime/core/src/loop_engine/context.rs` 的 `ContextManager` 提供 tiktoken-based token 估算和基础截断，Plan 模式循环中已接入。

---

## I. Workflow Engine / Agent Team

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| Plan 创建执行 | `docs/modules/workflow_engine.md` create_plan | TUI `plans.rs` 使用硬编码假数据 | 后端未实现 |
| Agent Team 协作 | `docs/products/third_party_agent_app.md` | TUI `agent_team.rs` UI 已有渲染 | 后端未实现 |
| A2A 消息处理 | `docs/modules/protocol_interface_layer.md` A2A | — | 未实现 |
| 多 Agent 网络层 | `docs/products/third_party_agent_app.md` | — | 未实现 |

---

## J. 协议适配器

| 适配器 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| Direct Rust API | `docs/modules/protocol_interface_layer.md` | `protocol/src/interface.rs` | 已实现 |
| JSON-RPC | `docs/modules/protocol_interface_layer.md` | `entrypoints/jsonrpc/src/lib.rs` | 最小序列化适配已建立 |
| C ABI FFI | `docs/modules/protocol_interface_layer.md` | — | 未实现 |
| A2A Protocol | `docs/modules/protocol_interface_layer.md` | — | 未实现 |

---

## K. TUI 完整接入

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| TUI 消费 CoreEvent stream | `docs/products/cli.md` | TUI workspace `collect_model_response` | **已实现** — 优先走 ProtocolBridge，降级回退直调 LlmClient |
| Event reducer | `docs/overview/DATA_FLOW.md` | — | 未实现 |

---

## L. `alius run` 非交互路径

| 项目 | 设计文档 | 代码位置 | 状态 |
| --- | --- | --- | --- |
| RunLoop 接入 | `docs/products/cli.md` | CLI `run` 命令 | **已实现** — 走 ProtocolBridge.send_message_streaming_with_mode() |

---

## 实现优先级建议

按对主路径影响排序：

1. **Logging Manager MVP**（D）— 日志是可观测性基础
2. **三层记忆系统**（C）— 长期上下文能力
3. **Event reducer**（K）— TUI 完整 CoreEvent 消费
4. 其余模块按需求排期
