# Alius Core Rewrite Specification

更新时间: 2026-06-06 09:15

## 规格定位

本文是 `.alius/workspace/docs/` 设计书的需求源头。所有功能点必须能映射到模块详细设计文档，并能被代码实现和测试验证。

## 功能规格

### F-001 项目目录初始化

需求:

- `alius init` 创建 `.alius/config/`、`.alius/memory/`、`.alius/workspace/`。
- legacy 路径只作为兼容读取，不作为新项目首选写入路径。

对应模块:

- `docs/modules/config_manager.md`
- `docs/modules/workspace_handler.md`
- `docs/modules/memory_manager/README.md`

验收:

- 新项目初始化后目标目录完整。
- `.alius/config/config.toml` 和 `.alius/config/mcp.json` 存在。
- `.alius/workspace/HISTORY.md` 存在并可追加修改记录。

### F-002 Protocol Interface Layer

需求:

- 所有产品入口通过协议接口层进入 Core Runtime。
- CLI / TUI 使用 Local Rust Interface，但不能绕过协议层。
- 协议层统一定义 `ProtocolEnvelope`、`CoreRequest`、`CoreCommand`、`CoreEvent`、`ProtocolError`。
- 普通聊天和 Plan 模式都必须表示为 `CoreRequest::RunLoop`，差异只由 `RuntimeMode` 和 `LoopPolicy` 表达。

对应模块:

- `docs/modules/protocol_interface_layer.md`
- `docs/modules/core_runtime.md`
- `docs/interfaces/protocol_interface_layer.md`

验收:

- 一次 CLI / TUI turn 可表示为 `ProtocolEnvelope<CoreRequest>`。
- 执行中审批、回答、取消可表示为 `ProtocolEnvelope<CoreCommand>`。
- TUI 消费 `ProtocolEnvelope<CoreEvent>`。
- Phase 0 最小 Rust 契约位于 `crates/protocol/src/core.rs`。
- `CoreRequestKind` 覆盖 RunLoop、session、config、memory、tool、review、health 全部操作（21 种）。
- `CoreCommandKind` 覆盖 Plan 审批、Review、模型/模式切换（12 种）。
- `CoreEventKind` 覆盖 loop iteration、convergence、approval、TUI workspace 所有渲染需求（34 种）。
- `CoreRuntimeApi` trait 定义 20 个方法覆盖全部产品层操作。
- `protocol-interface` crate 已落地 Direct Rust API 最小网关，可执行 start/send/subscribe/cancel 并输出 `ProtocolEnvelope<CoreEvent>`。
- 12 个非 start 委托方法已落地 ProtocolInterface（config_read/update、model_list、health_check、close_session、clear_conversation、memory_save/list/clear、tool_list、review_start、query_logs）。
- `RunLoopInput`、`RuntimeMode`、`LoopPolicy`、`ConvergenceDecision` 已落地协议层。
- CLI /memory、/tools、/review、/session clear 和 Chat 模式已改走 ProtocolBridge → ProtocolInterface → CoreRuntime 路径。
- TUI collect_model_response() 和 REPL Plan 模式已改走 ProtocolBridge。
- `alius run` 已从 chat_once() 改为 ProtocolBridge.send_message_streaming_with_mode()。

### F-003 Core Runtime 主路径

需求:

- Core Runtime 提供统一 `Core Public API`。
- 默认执行路径为 `Core Public API -> Session Manager -> Loop Engine`。
- TUI 不直接调用 provider stream。
- Workspace 是一个确定工程目录；Session 是该 workspace 下的一次开发轮次、功能开发、修复或长期任务。
- Core Runtime 必须初始化日志上下文，并将 session、run、trace 与日志关联。

对应模块:

- `docs/modules/core_runtime.md`
- `docs/modules/session_manager.md`
- `docs/modules/loop_engine.md`
- `docs/overview/ENGINEERING_BASELINE.md`

验收:

- 一次完整 turn 输出统一 `CoreEvent` stream。
- session、prompt、memory、model、tool、policy、budget、trace 事件可观测。
- workspace/session/run/trace/log 的关系可查询。
- Core Public API 实现必须满足 `protocol_interface::CoreRuntimeApi`。
- `core-runtime` crate 已落地 `CoreRuntimeApi` 实现和 SessionManager MVP。
- `core-runtime` crate 已新增 `loop_engine/` 结构和最小 loop lifecycle event。
- `core-runtime` crate 已实现全部 20 个 `CoreRuntimeApi` 方法，接入 MemoryStore、ToolRegistry、ConversationStore 子系统。

### F-004 三层记忆系统

需求:

- 项目记忆拆分为 episodic、semantic、procedural。
- Retrieval Engine 支持关键词和向量融合检索。
- legacy `.alius/memory/project.json` 可迁移。
- 情景记忆记录有时间边界的 session、turn、event、工具调用和用户决策，不默认等同于完整原始聊天日志。
- 用户输入原文是否保存由 retention/privacy policy 决定，可保存摘要、引用或脱敏内容。

对应模块:

- `docs/modules/memory_manager/README.md`
- `docs/modules/memory_manager/episodic_memory.md`
- `docs/modules/memory_manager/semantic_memory.md`
- `docs/modules/memory_manager/procedural_memory.md`
- `docs/modules/retrieval_engine.md`

验收:

- 新写入内容按类型进入对应记忆层。
- 检索结果包含 `content`、`score`、`memory_type`。
- semantic 不可用时可降级为 keyword retrieval。
- 情景记忆可按 trace_id 重建事件时间线。

### F-005 文档驱动开发

需求:

- `.alius/workspace/docs/modules/` 是模块实现的设计标准。
- 每次文档修改追加 `HISTORY.md`。
- 架构图、数据流图、模块流程图和实体关系图必须以 Markdown Mermaid 为主来源；`assets/` 只保存导出产物和附件。
- `ROADMAP.md` 不作为实现依据，最新实现依据是 `SPEC.md` 和 `docs/` 下的产品、接口、模块和规范文档。

对应模块:

- `docs/modules/workspace_handler.md`

验收:

- 每个模块文档包含职责、接口、内部逻辑、数据存储、异常处理、关系、验收标准。
- 文档更新后 `HISTORY.md` 有追加记录。

### F-006 Config 能力分层

需求:

- `.alius/config/` 拆分项目主配置、provider、tools、permissions、protocol、MCP。
- 除 MCP 标准配置 `mcp.json` 外，项目配置源文件统一使用 TOML；MCP 配置保持默认 JSON 格式。
- 项目 Agent Card 兼容信息记录在 `.alius/config/soul.toml`，不创建项目级 soul 目录；legacy soul 只能作为导入来源。
- `.alius/config/soul.toml` 是 TOML 源配置，保留 description、provider、skills、capabilities、交互模式，以及发布后才能确定的 supported interface URL、documentation URL、icon URL 等字段；A2A 发布时再导出为 Agent Card JSON。
- feature、origin、capability、policy 能在配置层表达。
- Config Manager 必须输出 RuntimeConfigView、LoggingConfig 和 ShellGateConfig。
- 日志路径固定为 `.alius/memory/logs/`，不由项目配置指定。
- 用户配置入口后续完善，但配置源文件边界先固定。

对应模块:

- `docs/modules/config_manager.md`
- `docs/modules/protocol_interface_layer.md`

验收:

- Config Manager 能生成 Core Runtime 所需的项目配置快照。
- Config Manager 能生成 Core Runtime 所需的运行时配置视图。
- Config Manager 能读取 `soul.toml` 并归一化为 A2A Agent Card 字段视图，包括 agent identity、supportedInterfaces、capabilities、skills、交互模式和 security schemes。
- `core-runtime` crate 已落地 embedded defaults（config/providers/soul/tools/permissions/protocol.toml 和 mcp.json）和项目配置加载/重置/目录结构创建。
- RemoteA2A、Embedded SDK、LocalTui 的默认能力边界不同。
- Logging Manager 和 Shell Gate 有默认配置。
- Logging Manager 使用固定日志路径，配置只控制开关、级别、脱敏和 flush 策略。

### F-007 GitFlow 工程提交与 PR 门禁

需求:

- 使用简化 GitFlow 管理研发、发布和线上修复。
- `master` 只代表稳定可发布状态，禁止直接提交研发改动。
- `develop` 作为下一版本集成线，禁止直接提交研发改动。
- 所有功能、修复、文档和工程配置变更必须先创建独立 feature/fix/docs/chore 分支，并默认合入 `develop`。
- release 分支从 `develop` 创建，命名为 `release/<version>`；只有 `release/*` 分支允许触发发布 CI，`release/` 后缀作为 GitHub Release tag/name，包版本按去掉可选前导 `v` 后的 SemVer 校验，完成发布验证后合入 `master` 并回流 `develop`。
- hotfix 分支从 `master` 创建，合入 `master` 后必须回流 `develop`。
- 本地 format、代码检查、lint 和单元测试全部通过后，才能创建 PR。
- 工作分支的整体检查和 Review 通过后，才能合入目标分支。

对应规范:

- `docs/standards/CODE_STANDARDS.md`
- `docs/standards/GITFLOW_WORKFLOW.md`

验收:

- 任一 PR 均能追溯到独立工作分支。
- PR 描述中记录本地检查命令和结果。
- 日常 feature/fix/docs/chore PR 默认合入 `develop`。
- release/hotfix PR 合入 `master` 后有回流 `develop` 的记录。
- `master` 始终保持可构建、可测试、可发布。

### F-008 架构细节完整性

需求:

- workspace 文档必须完整覆盖 Mermaid 架构图中的所有稳定节点和稳定连线。
- `Product Layer`、`Protocol Interface Layer`、`Core Runtime`、`External Resources`、`Build Targets / Cargo Features` 都必须有 workspace 对照文档。
- 核心术语、产品设计、技术选型、分层接口必须有独立文档。
- Core Runtime 中每个可实现模块都必须在 `docs/modules/` 下有独立详细设计或明确归属文档。
- 架构图中的连接关系必须记录 From、To、标签和工程含义。
- 数据流图必须使用 Mermaid flowchart 或 sequenceDiagram。
- 实体关系图必须使用 Mermaid erDiagram。

对应文档:

- `docs/README.md`
- `docs/terms/GLOSSARY.md`
- `docs/products/README.md`
- `docs/technology/TECHNOLOGY_SELECTION.md`
- `docs/interfaces/README.md`
- `docs/overview/DIAGRAMS.md`
- `docs/overview/ARCHITECTURE_DETAILS.md`
- `docs/overview/ARCH.md`
- `docs/overview/DATA_FLOW.md`
- `docs/overview/ENTITY_RELATIONSHIP.md`
- `docs/overview/BUILD_FEATURES.md`
- `docs/overview/EXTERNAL_RESOURCES.md`

验收:

- Mermaid 总架构图中的每个稳定节点都能在 `ARCHITECTURE_DETAILS.md` 找到。
- Mermaid 总架构图中的每条稳定连线都能在 `ARCHITECTURE_DETAILS.md` 找到。
- 每个 Core Runtime 模块都有对应模块文档。
- 每个产品入口都有产品文档和接口映射。
- 新增架构细节时必须同步更新 `HISTORY.md`。

### F-009 Workspace 文档确认归档

需求:

- `.alius/workspace/` 是工作版本目录。
- `.alius/workspace/.archive/` 是已完成版本快照目录。
- 用户日常在工作版本中新增、编辑、删除文件。
- 需要确认差异时，对比工作版本和 `.archive/`。
- 工作版本确认定稿后，用工作版本完整覆盖 `.archive/`。
- `.archive/` 不创建版本号子目录；归档快照保留工作区相对路径，便于目录 diff 识别新增、删除和修改。

对应规范:

- `docs/standards/WORKSPACE_UPDATE_CONFIRMATION.md`
- `docs/modules/workspace_handler.md`

验收:

- `.alius/workspace/.archive/` 存在。
- 能识别工作版本相对已完成版本的新增、删除、修改。
- 确认归档后，工作版本与 `.archive/` 快照无差异。
- 每次确认归档都追加 `HISTORY.md`。

### F-010 统一核心术语

需求:

- workspace 必须有独立 Markdown 术语表。
- 术语表记录统一术语、English ID、定义和边界。
- 产品文档、接口文档、模块文档和代码命名应优先使用术语表中的主术语。

对应文档:

- `docs/terms/GLOSSARY.md`

验收:

- `Workspace`、`Project`、`Session`、`Turn`、`Run`、`Trace` 有清晰区分。
- `Shell Gate`、`Logging Manager`、`Agent Card`、`ProtocolEnvelope` 等核心术语有定义。

### F-011 产品文档

需求:

- 每个产品形态必须有独立产品文档。
- CLI 和嵌入式第三方 SDK 是两个独立产品，必须分别说明。
- Desktop 是规划产品，本工程只保留接口规划，不实现 Desktop 产品本体。
- CLI 产品文档必须包含基础命令、交互内命令、操作设计流程、用户设计流程和注意事项。

对应文档:

- `docs/products/cli.md`
- `docs/products/embedded_sdk.md`
- `docs/products/desktop_planning.md`
- `docs/products/ide_extension_planning.md`
- `docs/products/third_party_agent_app.md`

验收:

- 每个产品有定位、市场定位、使用方式、接口边界和验收标准。
- CLI 基础指令和交互内指令可作为实现对照。

### F-012 技术选型

需求:

- 按产品记录技术选型。
- CLI、Embedded SDK、Desktop、IDE、A2A 需要分别说明。
- Desktop 技术只做规划，Tauri/Electron 等候选确认前不得写成实现承诺。

对应文档:

- `docs/technology/TECHNOLOGY_SELECTION.md`

验收:

- 每个产品能找到主技术栈、接口、构建目标和状态。

### F-013 分层接口契约

需求:

- 每一层接口必须有详细文档。
- Product Layer、Protocol Interface Layer、Core Runtime API 必须定义输入、输出、边界、错误和验收。
- 产品到接口的映射必须用矩阵表达。

对应文档:

- `docs/interfaces/product_layer.md`
- `docs/interfaces/protocol_interface_layer.md`
- `docs/interfaces/core_runtime_api.md`
- `docs/interfaces/product_interface_matrix.md`

验收:

- 每个产品入口都有 origin、capability_scope 和 Core 入口说明。
- CoreEvent stream、CoreCommand、ProtocolEnvelope 的语义一致。

### F-014 Shell 门禁

需求:

- 所有 shell、process、git 类工具调用必须经过 Shell Gate。
- Shell Gate 必须检查命令、参数、cwd、路径、glob、symlink、重定向和作用范围。
- 作用范围不能超过当前 workspace；如需读写 workspace 外路径，必须授权。
- `rm -rf` 等 critical destructive 命令默认拒绝或强制审批。

对应模块:

- `docs/modules/shell_gate.md`
- `docs/modules/tool_executor.md`
- `docs/modules/security_policy_manager.md`

验收:

- `rm -rf /`、`rm -rf ~`、`rm -rf .`、`rm -rf *` 有拒绝测试。
- workspace 外读写触发 approval required 或 deny。
- RemoteA2A 和 Embedded SDK 默认不能使用 shell。

### F-015 日志记录

需求:

- 系统必须实时记录 runtime、error、exception、audit 和 trace 日志。
- 日志必须关联 workspace、session、run、trace。
- 错误、异常、审批和 Shell Gate 拒绝必须立即记录。
- 日志必须脱敏密钥、token 和敏感输入。

对应模块:

- `docs/modules/logging_manager.md`
- `docs/modules/storage_manager.md`
- `docs/modules/core_runtime.md`

验收:

- 日志可按 workspace、session、run、trace、level 查询。
- CLI/TUI 退出前 flush 日志。
- Logging Manager 不可用时降级并发出可诊断事件。
