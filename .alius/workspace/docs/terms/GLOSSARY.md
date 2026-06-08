# Alius Core Terminology

更新时间: 2026-06-05 03:43

## 术语表定位

本文件是 Alius workspace 文档的统一术语表。产品文档、接口文档、模块设计和代码命名应优先使用本表中的术语，新增核心概念前必须先更新本表。

## 命名规则

- 中文文档首次出现核心术语时使用 `中文术语（English ID）`。
- 代码、配置、协议字段优先使用 English ID。
- 同一概念只允许一个主术语。旧称、别名只能写入“边界与备注”，不能继续扩散。
- `Workspace`、`Project`、`Session`、`Turn`、`Run` 必须严格区分。

## 核心术语表

| 术语 | English ID | 定义 | 边界与备注 |
| --- | --- | --- | --- |
| Alius | Alius | 面向工程工作区的 AI Agent Runtime 与产品集合。 | 不是单一聊天窗口，也不是只服务 CLI 的库。 |
| 产品层 | Product Layer | CLI、IDE、嵌入式 SDK、Desktop 规划、第三方 Agent 应用等用户或外部系统入口。 | 只负责体验、输入输出和产品形态，不直接承载 Core Runtime 业务逻辑。 |
| 协议接口层 | Protocol Interface Layer | 产品层进入 Core Runtime 的唯一工程边界。 | 负责传输适配、统一 envelope、origin 和 capability 归一化。 |
| 核心运行时 | Core Runtime | Alius 的统一执行层。 | 负责 session、loop engine、prompt、memory、model、tool、policy、budget、logging、trace 和 storage。 |
| Core 公开接口 | Core Public API | Core Runtime 对协议接口层暴露的唯一公开 API。 | 产品层不得直接调用 Core 内部模块。 |
| Core Runtime API trait | CoreRuntimeApi | 代码中表示 Core Public API 最小契约的 Rust trait。 | 定义在 `protocol_interface::core`，后续 Core Runtime 实现必须满足。 |
| 工作区 | Workspace | 一个确定工程目录对应的 Alius 工作范围。 | 一个 workspace 只对应一个工程目录，不用于管理多个工程。 |
| 工程 | Project | 被 Alius 管理和理解的实际代码或文档项目。 | 在当前设计中与 workspace 目录一一对应。 |
| Session | Session | 一次开发轮次、一个功能开发过程、一次长期任务或一次可恢复的工作上下文。 | Session 不等于 workspace。一个 workspace 下可有多个 session。 |
| Turn | Turn | Session 中一次用户输入到 CoreEvent 输出完成的交互轮次。 | Turn 是 session 的时间片。 |
| Run | Run | 一次 Core 执行实例，可被取消、审批、恢复或查询。 | Run 通常由一个 turn 启动，可有稳定 `run_ref`。 |
| Run 引用 | RunRef | 定位运行中或历史 run 的稳定引用。 | 用于 command、cancel、approval、inspect。 |
| Trace | Trace | 贯穿一次 run 的诊断链路。 | 与日志不同，trace 更偏因果链和事件链。 |
| Core 请求 | CoreRequest | 协议层归一化后提交给 Core 的请求。 | 表示启动 turn、打开 session、查询状态等意图。 |
| Core 命令 | CoreCommand | 执行中的控制命令。 | 例如取消、审批、继续、暂停某个 run。 |
| Core 事件 | CoreEvent | Core Runtime 对外输出的统一事件流。 | TUI、CLI、A2A、JSON-RPC 都应消费同一语义。 |
| 协议信封 | ProtocolEnvelope | 包装 CoreRequest、CoreCommand、CoreEvent 的协议消息容器。 | 必须携带 origin、capability_scope、trace_id、protocol_version。 |
| 协议错误 | ProtocolError | 协议层和 Core Public API 共享的错误类型。 | 包含 UnsupportedVersion、InvalidMessage、CapabilityDenied、RunNotFound 等。 |
| Origin | Origin | 请求来源身份。 | 例如 LocalTui、IDE、Desktop、RemoteA2A、Embedded SDK。 |
| 能力范围 | Capability Scope | 某个 origin 在当前请求中允许使用的能力上限。 | 是权限判断的输入，不是最终授权结果。 |
| 能力项 | Capability | 能力范围中的单项能力声明。 | 例如 ReadWorkspace、WriteWorkspace、UseShell、UseMcp。 |
| Config Manager | Config Manager | 读取、合并、校验、迁移配置并生成运行时配置快照的模块。 | Core Runtime 的重要基础模块。 |
| 项目配置快照 | ProjectConfigSnapshot | Core Runtime 启动和执行时使用的合并后配置视图。 | 由默认配置、用户配置、项目配置、环境变量、CLI 参数合并得到。 |
| Soul 配置 | SoulConfig | `.alius/config/soul.toml` 中保存的 Agent Card 兼容 TOML 源配置。 | 不再创建独立项目级 soul 文件夹。 |
| Agent Card 视图 | AgentCardView | 由 `soul.toml` 归一化得到的 A2A Agent Card 字段视图。 | 发布时可导出为 `.well-known/agent-card.json`。 |
| Agent Card | Agent Card | A2A 协议中的 Agent 描述、能力和 skill 声明。 | Alius 项目内以 `soul.toml` 作为源配置。 |
| Alius CLI | Alius CLI | 当前主产品，面向指定 workspace 的命令行和 TUI 工具。 | 通过 Direct Rust API 进入协议接口层。 |
| TUI | TUI | CLI 内的全屏终端交互工作区。 | 定位为 Plan-driven Agent Runtime Workspace。 |
| 嵌入式第三方 SDK | Embedded Third Party SDK | 通过 C ABI FFI 暴露给嵌入式或第三方宿主的裁剪 SDK。 | 使用 Embedded Core Lite，不包含完整本地工具运行时。 |
| Desktop 规划产品 | Desktop Planning Product | 未来桌面端产品规划。 | 本工程只实现相关接口规划或接口基础，不交付 Desktop 产品本体。 |
| IDE 插件规划产品 | IDE Extension Planning Product | VS Code、JetBrains 等编辑器集成规划。 | 通过 Plugin RPC Adapter 接入。 |
| 第三方 Agent 应用 | Third Party Agent App | 通过 A2A 与 Alius 协作的外部 Agent 应用。 | 默认不得继承本地文件和 shell 权限。 |
| Session Manager | Session Manager | 管理 workspace 内多个 session、turn、run、context 和 task state 的模块。 | 用于开发轮次和长期任务恢复。 |
| Loop Engine | Loop Engine | Core Runtime 内统一承载 Chat Mode 与 Plan Mode 的循环执行引擎。 | 负责模型调用、工具调用、事件流和收敛判断；Chat 只是 `max_iterations=1` 的受限 loop。 |
| RuntimeMode | RuntimeMode | 表示运行时交互模式的协议枚举。 | 当前包含 `Chat` 和 `Plan`，由 REPL/TUI/产品入口传入。 |
| ReplMode | ReplMode | CLI REPL 层的本地模式状态。 | 通过 Shift+Tab、`/mode chat`、`/mode plan` 切换，并映射为 `RuntimeMode`。 |
| LoopPolicy | LoopPolicy | 控制 Loop Engine 行为的协议策略。 | 包含最大迭代次数、工具开关、规划开关、收敛检查和工具审批要求。 |
| RunLoopInput | RunLoopInput | `CoreRequest::RunLoop` 的输入结构。 | 包含用户输入内容、`RuntimeMode` 和 `LoopPolicy`。 |
| ConvergenceDecision | ConvergenceDecision | 每次 loop 迭代后的收敛判定结果。 | 包含 Continue、Completed、NeedUserInput、NeedApproval、Failed、MaxIterationsReached。 |
| Workflow Engine | Workflow Engine | Plan、Todo、Task Orchestration 的编排模块。 | 用户可见术语优先使用 Plan，不使用 Task 替代 Plan。 |
| 记忆系统 | Memory System | episodic、semantic、procedural 三层记忆和检索能力。 | 不是原始聊天日志的简单堆叠。 |
| 情景记忆 | Episodic Memory | 保存有时间边界的 session、turn、事件、决策和工具调用证据。 | 用户输入可保存引用或摘要，是否保存原文受隐私和策略控制。 |
| 语义记忆 | Semantic Memory | 保存稳定项目事实、设计决策、文档分片和向量索引。 | 关注可复用事实，不关注完整时间线。 |
| 程序记忆 | Procedural Memory | 保存可复用流程、规则、playbook 和失败模式。 | 关注“怎么做”，不是“发生过什么”。 |
| 检索引擎 | Retrieval Engine | 关键词和向量融合检索并返回排序结果的模块。 | semantic 不可用时必须降级为关键词检索。 |
| Shell 门禁 | Shell Gate | 对 shell/process/git 等本地命令进行命令、参数和作用范围检查的门禁模块。 | `rm -rf` 等高风险删除默认拒绝；越过 workspace 的读写必须授权。 |
| 工具执行器 | Tool Executor | 文件、Shell、Web、MCP、Agent/Task 等工具的统一执行入口。 | Shell 工具必须先经过 Shell Gate 和 Security Policy。 |
| 安全策略管理器 | Security And Policy Manager | 审批、权限、sandbox、allowlist 和 deny 策略统一边界。 | 策略冲突默认 deny。 |
| 日志管理器 | Logging Manager | 实时记录系统运行日志、异常、错误和审计事件的模块。 | 日志用于诊断，trace 用于链路，二者可关联。 |
| 存储管理器 | Storage Manager | 配置、会话、缓存、trace、日志和 keychain 的统一存储访问。 | 必须保证命名空间隔离。 |
| MCP 配置 | MCP Config | `.alius/config/mcp.json` 中的 MCP server 声明。 | MCP 标准默认 JSON，不迁移成 TOML。 |
| A2A 适配器 | A2A Adapter | A2A Server/Client、Task Mapper、Remote Registry 的 Core 内部模块。 | 由产品开关和 Soul 策略启用。 |
| Direct Rust API | Direct Rust API | CLI/TUI 同进程进入协议层的本地 Rust 接口。 | 仍属于协议接口层，不等于绕过协议层。 |
| C ABI FFI Adapter | C ABI FFI Adapter | 嵌入式 SDK 对 C 宿主暴露的 FFI 接口。 | 输出结构必须 FFI-safe。 |
| JSON-RPC Adapter | JSON-RPC Adapter | Desktop 规划接入的 RPC 适配。 | 当前已建立 `entrypoints/jsonrpc/` 最小序列化适配器。 |
| Plugin RPC Adapter | Plugin RPC Adapter | IDE 插件接入的 RPC 适配。 | JSON-RPC / LSP-like，stdio 或 socket。 |
| 配置快照 | ConfigSnapshot | Core Runtime 返回的当前运行时配置视图。 | 包含 provider、model、base_url、soul、has_api_key。 |
| 校验结果 | ValidationResult | 配置完整性校验结果。 | 包含 valid 标志和 errors 列表。 |
| 模型信息 | ModelInfo | Provider 提供的可用模型描述。 | 包含 id 和 name。 |
| 记忆条目 | MemoryEntry | 记忆系统中的一条记录。 | 包含 id、content、tags、created_at。 |
| 工具信息 | ToolInfo | 可用工具的描述。 | 包含 name、description 和 source（BuiltIn/Mcp/Plugin）。 |
| 工具来源 | ToolSource | 工具的来源类型。 | BuiltIn（内置）、Mcp（MCP server）、Plugin（WASM 插件）。 |
| 健康报告 | HealthReport | 系统健康检查结果。 | 包含 config_ok、api_reachable、workspace_ok 和 errors。 |
| 内嵌默认配置 | Embedded Defaults | 编译进 `core-runtime` 二进制的默认配置模板。 | 包含 config/providers/soul/tools/permissions/protocol.toml 和 mcp.json，用于 `alius init` 初始化项目。 |
| Config Loader | Config Loader | `runtime/core/src/config.rs` 中负责配置加载、重置和项目目录结构创建的模块。 | 当前已实现 embedded defaults 输出和项目结构创建，完整分层合并仍为 H1 任务。 |

## 待确认术语

| 待确认项 | 当前处理 |
| --- | --- |
| `Tears` | 按用户上下文暂理解为 Desktop/Tauri 或桌面技术选型候选，在技术选型文档中标记为待确认，不作为当前实现承诺。 |
