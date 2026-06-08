# Alius Architecture Details

更新时间: 2026-06-05 03:43

## 图表来源

本文件是整体架构的逐项核对文档，覆盖 Markdown Mermaid 架构图中的每个稳定节点和每条连线。后续 workspace 图表以 Markdown Mermaid 为主来源，不再依赖外部绘图文件作为设计图维护源。

```text
canonical diagram source: .alius/workspace/docs/overview/DIAGRAMS.md
canonical detail source: .alius/workspace/docs/overview/ARCHITECTURE_DETAILS.md
```

## 架构分层

```text
Product Layer
-> Protocol Interface Layer
-> Core Runtime
-> External Resources
-> Build Targets / Cargo Features
```

说明:

- Product Layer 是产品入口层，负责用户体验、输入输出和产品形态。
- Protocol Interface Layer 是所有产品入口到 Core Runtime 的唯一工程边界。
- Core Runtime 是统一执行层，负责 session、loop engine、prompt、memory、model、tool、policy、budget、trace 和 storage。
- External Resources 不是架构层，只是 Core Runtime 的外部依赖。
- Build Targets / Cargo Features 定义不同产品形态的编译边界。

## 节点清单

### Mermaid Architecture IDs

| ID | 分组 | 名称 | 图中细节 | Workspace 对应文档 |
| --- | --- | --- | --- | --- |
| `product_layer` | Product | 产品层 Product | 顶层产品入口分组 | `overview/ARCH.md` |
| `interface_layer` | Interface | 接口层 Interface | 产品到 Core 的协议边界分组 | `modules/protocol_interface_layer.md` |
| `core_layer` | Core Runtime | Core Runtime | 统一核心运行时分组 | `modules/core_runtime.md` |
| `external_group` | External Resources | 外部资源 External Resources | 不是架构层，只是 Core Runtime 的外部依赖 | `overview/EXTERNAL_RESOURCES.md` |
| `build_group` | Build / Features | 编译目标 / Cargo Features | 编译目标和 feature policy 分组 | `overview/BUILD_FEATURES.md` |
| `p_cli` | Product | Alius CLI（含 TUI） | 当前主产品 | `products/cli.md` |
| `p_ide` | Product | IDE 插件 | VS Code / JetBrains | `products/ide_extension_planning.md` |
| `p_embedded` | Product | 嵌入式第三方库 / SDK | ESP32 + LVGL | `products/embedded_sdk.md` |
| `p_desktop` | Product | Desktop 应用 | Electron，规划产品 | `products/desktop_planning.md` |
| `p_third_agent` | Product | 第三方 Agent 应用 | 通过 A2A 与 Alius 通信 | `products/third_party_agent_app.md` |
| `i_rust` | Interface | Direct Rust API | CLI 同进程函数调用 | `interfaces/protocol_interface_layer.md` |
| `i_plugin` | Interface | Plugin RPC Adapter | JSON-RPC / LSP-like；stdio / socket | `interfaces/protocol_interface_layer.md` |
| `i_ffi` | Interface | C ABI FFI Adapter | 嵌入式 SDK 唯一保留接口 | `interfaces/protocol_interface_layer.md` |
| `i_jsonrpc` | Interface | JSON-RPC Adapter | Desktop 规划接入；stdio / socket | `interfaces/protocol_interface_layer.md` |
| `i_a2a` | Interface | A2A Protocol Adapter | 接口标准实现；CLI / Desktop 可开关启用，第三方 Agent 应用通过该接口通信 | `interfaces/protocol_interface_layer.md` |
| `interface_bus` | Interface | Interface -> Core Entry Bus | 所有接口进入 Core 的统一入口总线 | `interfaces/protocol_interface_layer.md` |
| `c_coreapi` | Core Runtime | Core Public API | 统一核心入口 | `modules/core_runtime.md` |
| `c_session` | Core Runtime | Session Manager | Thread / Turn / Context / Task State | `modules/session_manager.md` |
| `c_engine` | Core Runtime | Loop Engine | 主循环 / Tool-call Loop / Event Stream | `modules/loop_engine.md` |
| `m_prompt` | Core Runtime | Prompt Builder | L1 基础身份；L2 运行时环境注入；L3 用户附加规则；L4 角色和模式附加；L5 工具 / 后台专用提示词 | `modules/prompt_builder.md` |
| `m_soul` | Core Runtime | Soul Manager | Active Soul；Prompt / Policy / Agent Card Source | `modules/soul_manager.md` |
| `m_memory` | Core Runtime | Memory System | 语义检索 / 记忆召回 / 写回 | `modules/memory_manager/README.md` |
| `m_a2a` | Core Runtime | A2A Adapter | A2A Server + Client；Task Mapper / Remote Registry；由产品开关启用，由 Soul 策略驱动 | `modules/a2a_adapter.md` |
| `m_model` | Core Runtime | Model Router | 三层模型分流；light / medium / high | `modules/model_router.md` |
| `m_provider` | Core Runtime | Provider Manager | OpenAI / Anthropic / Google；BigModel / Custom | `modules/provider_manager.md` |
| `m_tool` | Core Runtime | Tool Executor | 文件 / 环境 / 会话 / 协作工具；Read / Edit / Write / Grep；Bash / WebFetch / WebSearch；AskUserQuestion / Plan / Todo；Agent / Task / MCP Resource | `modules/tool_executor.md` |
| `m_shell_gate` | Core Runtime | Shell Gate | Shell / process / git 命令、参数和作用范围门禁；workspace 外读写需授权；destructive 命令默认拒绝或强制审批 | `modules/shell_gate.md` |
| `m_workflow` | Core Runtime | Workflow Engine | Plan / Todo / Task Orchestration | `modules/workflow_engine.md` |
| `m_store` | Core Runtime | Storage Manager | 配置 / 会话 / 缓存 / Trace / Keychain | `modules/storage_manager.md` |
| `m_security` | Core Runtime | Security & Policy Manager | 审批 / 权限 / Sandbox / Allowlist | `modules/security_policy_manager.md` |
| `m_logging` | Core Runtime | Logging Manager | Runtime / Error / Exception / Audit log；按 workspace、session、run、trace 关联 | `modules/logging_manager.md` |
| `m_budget` | Core Runtime | Budget Manager | Token Budget 超限终止；连续失败 >= 3 次熔断；时间 / 成本 / 工具调用预算 | `modules/budget_manager.md` |
| `m_context` | Core Runtime | Context Manager | 上下文窗口跟踪；截断 / 摘要 / 引用组织 | `modules/context_manager.md` |
| `m_compress` | Core Runtime | Compression Worker | 后台压缩上下文；预留 20,000 tokens 压缩空间 | `modules/compression_worker.md` |
| `m_core_lite` | Core Runtime | Embedded Core Lite | 仅保留部分 Core 功能；配置 / 模型路由 / 远程调用 / 轻量记忆缓存；禁用本地工具运行时、LanceDB、本地 embedding、重型插件 | `modules/embedded_core_lite.md` |
| `m_a2a_switch` | Core Runtime | A2A Enable Switch | CLI: `--a2a` / config；Desktop: settings toggle；第三方 Agent: A2A 标准入口；Embedded SDK: 默认关闭 | `modules/a2a_adapter.md` |
| `d_soul` | Data | Installed Soul Data | `soul.toml` / `agent-card.json`；prompts / `runtime.toml` / `a2a.toml` | `modules/soul_manager.md` |
| `d_memory` | Data | Memory Data | docs / LanceDB / SQLite；embedding cache | `modules/memory_manager/README.md` |
| `d_logs` | Data | Runtime Logs Audit Logs | `.alius/memory/logs/runtime.log.jsonl`；`error.log.jsonl`；`audit.log.jsonl`；trace log | `modules/logging_manager.md` |
| `x_model` | External | Model Provider APIs | 外部模型供应商 API | `overview/EXTERNAL_RESOURCES.md` |
| `x_embed` | External | Embedding APIs / Memory Gateway | embedding API 或外部 memory gateway | `overview/EXTERNAL_RESOURCES.md` |
| `x_local` | External | Local OS Capabilities | File / Process / Shell / Git | `overview/EXTERNAL_RESOURCES.md` |
| `x_mcp` | External | External MCP Servers | 外部 MCP Server | `overview/EXTERNAL_RESOURCES.md` |
| `x_a2a` | External | Remote A2A Agents / Apps | 外部协议对端 | `overview/EXTERNAL_RESOURCES.md` |
| `x_soulrepo` | External | Official Soul Repository | AliusTech/alius-souls | `overview/EXTERNAL_RESOURCES.md` |
| `b_cli` | Build / Features | CLI 构建 | `cargo build --release`；features = `cli-full/default`；完整 Core Runtime；A2A 可运行时开关启用 | `overview/BUILD_FEATURES.md` |
| `b_embedded` | Build / Features | 嵌入式第三方库 / SDK 构建 | `cargo build --release --features embedded-sdk`；crate-type = staticlib / cdylib；只编译 FFI + Core Lite；A2A 默认不编译或关闭 | `overview/BUILD_FEATURES.md` |
| `b_policy` | Build / Features | Feature Policy | `cli-full`: tools + memory-standard + optional a2a + local embedding；`desktop-planned`: json-rpc + optional a2a；`embedded-sdk`: ffi + core-lite + remote model/embedding；禁用 heavy tools / LanceDB / local embedding / plugin runtime | `overview/BUILD_FEATURES.md` |

## 连线清单

| ID | From | To | 标签 | 含义 |
| --- | --- | --- | --- | --- |
| `e_prod_cli` | `p_cli` | `i_rust` |  | CLI / TUI 默认通过 Direct Rust API 同进程进入接口层 |
| `e_prod_ide` | `p_ide` | `i_plugin` |  | IDE 插件通过 Plugin RPC Adapter 接入 |
| `e_prod_emb` | `p_embedded` | `i_ffi` |  | 嵌入式第三方库 / SDK 通过 C ABI FFI 接入 |
| `e_prod_desktop` | `p_desktop` | `i_jsonrpc` | planned | Desktop 规划通过 JSON-RPC Adapter 接入 |
| `e_prod_3rd_a2a` | `p_third_agent` | `i_a2a` | A2A communication | 第三方 Agent 应用通过 A2A 标准与 Alius 通信 |
| `e_cli_a2a` | `p_cli` | `i_a2a` | optional enable | CLI 可通过开关启用 A2A |
| `e_desktop_a2a` | `p_desktop` | `i_a2a` | planned optional | Desktop 规划可选启用 A2A |
| `e_i_rust_bus` | `i_rust` | `interface_bus` |  | Direct Rust API 进入 Interface -> Core Entry Bus |
| `e_i_plugin_bus` | `i_plugin` | `interface_bus` |  | Plugin RPC Adapter 进入统一接口总线 |
| `e_i_ffi_bus` | `i_ffi` | `interface_bus` |  | FFI Adapter 进入统一接口总线 |
| `e_i_json_bus` | `i_jsonrpc` | `interface_bus` |  | JSON-RPC Adapter 进入统一接口总线 |
| `e_i_a2a_runtime` | `i_a2a` | `m_a2a` |  | A2A Protocol Adapter 先进入 Core 内部 A2A Adapter |
| `e_bus_core` | `interface_bus` | `c_coreapi` |  | 所有接口统一进入 Core Public API |
| `e_core_session` | `c_coreapi` | `c_session` |  | Core Public API 委派 Session Manager |
| `e_session_engine` | `c_session` | `c_engine` |  | Session Manager 启动 Loop Engine |
| `e_session_soul` | `c_session` | `m_soul` |  | Session Manager 读取 active soul / Agent Card 来源 |
| `e_session_memory` | `c_session` | `m_memory` |  | Session Manager 协调 memory 读取和写入 |
| `e_engine_model` | `c_engine` | `m_model` |  | Loop Engine 请求 Model Router |
| `e_model_provider` | `m_model` | `m_provider` |  | Model Router 将请求路由到 Provider Manager |
| `e_engine_tool` | `c_engine` | `m_tool` |  | Loop Engine 发起工具调用 |
| `e_tool_shellgate` | `m_tool` | `m_shell_gate` |  | Shell / process / git 工具调用先进入 Shell Gate 检查命令、参数和作用范围 |
| `e_engine_workflow` | `c_engine` | `m_workflow` |  | Loop Engine 进入 Plan / Todo / Task 编排 |
| `e_engine_store` | `c_engine` | `m_store` |  | Loop Engine 写入配置、会话、缓存、Trace 等存储 |
| `e_engine_security` | `c_engine` | `m_security` |  | Loop Engine 调用审批、权限、sandbox、allowlist |
| `e_engine_logging` | `c_engine` | `m_logging` |  | Loop Engine 记录 runtime、error、trace 和 audit 上下文 |
| `e_engine_budget` | `c_engine` | `m_budget` | checks | Loop Engine 每轮执行中检查预算和熔断 |
| `e_engine_context` | `c_engine` | `m_context` |  | Loop Engine 交由 Context Manager 管理上下文 |
| `e_context_compress` | `m_context` | `m_compress` | trigger | Context Manager 触发后台压缩 |
| `e_soul_prompt` | `m_soul` | `m_prompt` | drives | Soul Manager 驱动 Prompt Builder |
| `e_prompt_engine` | `m_prompt` | `c_engine` |  | Prompt Builder 输出 prompt 给 Loop Engine |
| `e_soul_a2a` | `m_soul` | `m_a2a` | agent card / policy | Soul Manager 提供 A2A Agent Card 和策略 |
| `e_a2aswitch_a2a` | `m_a2a_switch` | `m_a2a` | enable/disable | A2A Enable Switch 控制 A2A Adapter |
| `e_a2a_session` | `m_a2a` | `c_session` | map to Core Request | A2A Adapter 将远端 task 映射为 Core Request |
| `e_ffi_corelite` | `i_ffi` | `m_core_lite` |  | FFI Adapter 可以进入 Embedded Core Lite |
| `e_corelite_model` | `m_core_lite` | `m_model` | partial | Core Lite 只使用部分 Model Router 能力 |
| `e_corelite_memory` | `m_core_lite` | `m_memory` | light cache | Core Lite 只使用轻量 memory cache |
| `e_corelite_store` | `m_core_lite` | `m_store` | config | Core Lite 使用配置存储子集 |
| `e_corelite_security` | `m_core_lite` | `m_security` | subset | Core Lite 使用安全策略子集 |
| `e_soul_data` | `m_soul` | `d_soul` |  | Soul Manager 读取 Installed Soul Data |
| `e_memory_data` | `m_memory` | `d_memory` |  | Memory System 读写 Memory Data |
| `e_logging_data` | `m_logging` | `d_logs` |  | Logging Manager 写入运行日志、错误日志、异常日志和审计日志 |
| `e_session_logging` | `c_session` | `m_logging` |  | Session Manager 提供 workspace/session/run/trace 日志上下文 |
| `e_provider_logging` | `m_provider` | `m_logging` |  | Provider Manager 记录 provider request、fallback 和错误 |
| `e_tool_logging` | `m_tool` | `m_logging` |  | Tool Executor 记录工具执行、输出摘要和失败 |
| `e_shellgate_security` | `m_shell_gate` | `m_security` |  | Shell Gate 将命令检查结果交给 Security & Policy Manager 做最终授权 |
| `e_shellgate_logging` | `m_shell_gate` | `m_logging` |  | Shell Gate 记录 deny、approval required 和高风险命令审计 |
| `e_security_logging` | `m_security` | `m_logging` |  | Security & Policy Manager 记录审批、deny 和 policy conflict |
| `e_memory_context` | `m_memory` | `c_engine` | retrieved context | Memory System 将召回上下文返回 Loop Engine |
| `e_provider_api` | `m_provider` | `x_model` |  | Provider Manager 调用外部模型供应商 API |
| `e_memory_embed` | `m_memory` | `x_embed` |  | Memory System 调用 embedding API 或 memory gateway |
| `e_security_local` | `m_security` | `x_local` |  | 通过 Security / Policy 和 Shell Gate 授权后访问本地 OS 能力 |
| `e_tool_mcp` | `m_tool` | `x_mcp` |  | Tool Executor 调用外部 MCP Server |
| `e_a2a_remote` | `m_a2a` | `x_a2a` |  | A2A Adapter 调用远端 A2A agents / apps |
| `e_third_to_remote` | `p_third_agent` | `x_a2a` | external peer | 第三方 Agent 本身也是 Remote A2A peer |
| `e_soulrepo` | `d_soul` | `x_soulrepo` | sync/update | Installed Soul Data 从官方 Soul 仓库同步或更新 |
| `e_corelite_modelapi` | `m_core_lite` | `x_model` | remote inference | Core Lite 通过外部模型 API 远程推理 |
| `e_corelite_embedapi` | `m_core_lite` | `x_embed` | remote embedding/memory | Core Lite 通过远程 embedding / memory gateway 工作 |
| `e_build_cli` | `b_cli` | `p_cli` | build target | CLI 构建产物对应 Alius CLI / TUI |
| `e_build_emb` | `b_embedded` | `p_embedded` | build target | 嵌入式 SDK 构建产物对应嵌入式第三方库 / SDK |
| `e_build_ffi` | `b_embedded` | `i_ffi` | exports | embedded-sdk 构建导出 FFI Adapter |
| `e_build_corelite` | `b_embedded` | `m_core_lite` | compiles | embedded-sdk 构建只编译 Core Lite |
| `e_policy_cli` | `b_policy` | `b_cli` |  | Feature Policy 约束 CLI 构建 |
| `e_policy_emb` | `b_policy` | `b_embedded` |  | Feature Policy 约束 embedded-sdk 构建 |

## 实现边界约束

1. Product Layer 不直接调用 Core Runtime 内部模块，只能通过对应接口适配进入 Protocol Interface Layer。
2. Protocol Interface Layer 不执行业务逻辑，只做传输适配、统一 envelope、origin/capability 归一化和 Core Gateway。
3. Core Public API 是 Core Runtime 的唯一公开入口。
4. A2A Protocol Adapter 不是直接绕过 Core 的快捷通道，必须经 A2A Adapter 映射到 Core Request。
5. FFI Adapter 可以进入 Embedded Core Lite，但 Core Lite 必须是裁剪目标，不能泄漏完整 Core Runtime。
6. External Resources 不属于架构层，任何调用都必须经过 Core 模块和 Security / Policy 边界。
7. Build / Feature Policy 是架构边界的一部分，必须影响编译、模块注册、接口暴露和测试矩阵。
8. shell、process、git 类本地命令必须经过 Shell Gate；命令、参数和作用范围不能越过 workspace，除非获得授权。
9. runtime、error、exception、audit 日志必须由 Logging Manager 实时记录，并与 workspace、session、run、trace 关联。

## 当前口径修正

原图中 `m_soul` / `d_soul` 使用 Soul Manager、Installed Soul Data 的命名。当前 workspace 采用兼容命名:

- 项目内配置源为 `.alius/config/soul.toml`。
- `soul.toml` 保存 Agent Card-compatible 字段。
- 发布 A2A 服务时再导出 `.well-known/agent-card.json`。
- 不创建项目级 `.alius/soul/` 目录。
- legacy soul 只作为导入或同步来源。
