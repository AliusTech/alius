# Workspace Design Documents

更新时间: 2026-06-06 09:00

## 文档定位

本目录保存 Alius v10 重构的项目设计书。`terms/` 固定核心术语；`products/` 记录产品设计；`interfaces/` 记录分层接口契约；`technology/` 记录产品技术选型；`overview/` 负责整体架构、数据流和全局规则；`modules/` 负责模块级详细设计；`standards/` 负责工程流程与代码规范。

`.alius/workspace/` 是工作版本目录；`.alius/workspace/.archive/` 是已完成版本快照目录。文档确认和归档流程见 `standards/WORKSPACE_UPDATE_CONFIRMATION.md`。

## 工程目录约束

工程目录结构:

```text
entrypoints/
├── cli/        # alius-cli 产品入口
└── jsonrpc/    # alius-jsonrpc 适配器
protocol/       # Protocol Interface
runtime/
├── core/       # Core Runtime
├── config/     # Settings
├── model/      # LLM client
├── store/      # Storage
└── tools/      # Tool registry
```

原 `config/model/store/tools/formula/mcp/plugin/workflow` 不再作为独立 crate 存在。原 `protocol` 类型和 Direct Rust Protocol Interface 网关统一由 `protocol/` 承载。

## 图表来源

workspace 的架构图、数据流图和实体关系图统一以 Markdown Mermaid 为主来源，便于代码评审、AI 解析和版本 diff。

Mermaid 源文件:

```text
overview/DIAGRAMS.md
overview/DATA_FLOW.md
overview/ENTITY_RELATIONSHIP.md
```

完整节点和连线明细见:

```text
overview/ARCHITECTURE_DETAILS.md
```

## 文档结构

```text
docs/
├── README.md
├── terms/
│   └── GLOSSARY.md
├── products/
│   ├── README.md
│   ├── cli.md
│   ├── embedded_sdk.md
│   ├── desktop_planning.md
│   ├── ide_extension_planning.md
│   └── third_party_agent_app.md
├── technology/
│   └── TECHNOLOGY_SELECTION.md
├── interfaces/
│   ├── README.md
│   ├── product_interface_matrix.md
│   ├── product_layer.md
│   ├── protocol_interface_layer.md
│   └── core_runtime_api.md
├── overview/
│   ├── ARCH.md
│   ├── ARCHITECTURE_DETAILS.md
│   ├── BUILD_FEATURES.md
│   ├── DATA_FLOW.md
│   ├── DIAGRAMS.md
│   ├── ENGINEERING_BASELINE.md
│   ├── ENTITY_RELATIONSHIP.md
│   ├── EXTERNAL_RESOURCES.md
│   └── H1_EXECUTION_CHAIN_REMEDIATION.md
├── modules/
│   ├── a2a_adapter.md
│   ├── budget_manager.md
│   ├── compression_worker.md
│   ├── config_manager.md
│   ├── context_manager.md
│   ├── core_runtime.md
│   ├── embedded_core_lite.md
│   ├── logging_manager.md
│   ├── loop_engine.md
│   ├── memory_manager/
│   ├── model_router.md
│   ├── prompt_builder.md
│   ├── provider_manager.md
│   ├── retrieval_engine.md
│   ├── security_policy_manager.md
│   ├── session_manager.md
│   ├── shell_gate.md
│   ├── soul_manager.md
│   ├── storage_manager.md
│   ├── tool_executor.md
│   ├── workflow_engine.md
│   └── workspace_handler.md
└── standards/
    ├── CODE_STANDARDS.md
    ├── RELEASE_PROCESS.md
    └── WORKSPACE_UPDATE_CONFIRMATION.md
```

## 阅读顺序

1. `terms/GLOSSARY.md`: 统一核心术语。
2. `products/*.md`: 产品定位、使用方式、用户流程和注意事项。
3. `technology/TECHNOLOGY_SELECTION.md`: 各产品技术选型。
4. `interfaces/*.md`: 产品层、协议层和 Core Runtime API 契约。
5. `overview/ENGINEERING_BASELINE.md`: 当前代码路径、目标主路径、阶段冻结项和 H1 起点。
6. `overview/H1_EXECUTION_CHAIN_REMEDIATION.md`: Core Runtime 主链、REPL/TUI、权限、Shell Gate 和配置 schema 的 H1 纠偏方案。
7. `overview/DIAGRAMS.md`: 架构图、协议图、构建图和主要流程图的 Mermaid 源。
8. `overview/ARCHITECTURE_DETAILS.md`: Mermaid 稳定节点和连线核对表。
9. `overview/ARCH.md`: 三层架构总览。
10. `overview/DATA_FLOW.md`: 核心数据流 Mermaid 图。
11. `overview/ENTITY_RELATIONSHIP.md`: 核心实体关系 Mermaid ER 图。
12. `overview/BUILD_FEATURES.md`: Cargo feature 和构建目标策略。
13. `overview/EXTERNAL_RESOURCES.md`: 外部依赖边界。
14. `modules/*.md`: 按模块进入详细设计。
15. `standards/RELEASE_PROCESS.md`: 基于 Release Please 的自动化发版流程、Conventional Commits 规范、PAT 配置和故障排查。
16. `standards/CODE_STANDARDS.md`: 开发、检查、PR 和合并规范。
17. `standards/WORKSPACE_UPDATE_CONFIRMATION.md`: 工作版本、已完成版本、diff 和归档确认流程。

## Roadmap 口径

`ROADMAP.md` 不作为实现依据。Roadmap 会随研发推进变化，最新实现依据始终是 `SPEC.md`、`docs/` 下的产品、接口、模块和规范文档。

## 完整性规则

- Mermaid 架构图中的每个稳定节点必须能在 `overview/ARCHITECTURE_DETAILS.md` 找到。
- Mermaid 架构图中的每条稳定连线必须能在 `overview/ARCHITECTURE_DETAILS.md` 找到。
- 数据流图必须使用 Mermaid flowchart 或 sequenceDiagram。
- 实体关系图必须使用 Mermaid erDiagram。
- Core Runtime 中的每个可实现模块必须有对应 `modules/` 文档，容器、外部资源和构建策略除外。
- 每个含 `更新时间:` 的文档必须使用分钟级格式: `YYYY-MM-DD HH:MM`。
- 新增或调整模块时，必须同步更新 `SPEC.md`、`HISTORY.md` 和本索引；如影响产品或接口，必须同步更新 `products/` 和 `interfaces/`。
- Roadmap 只作为阶段性参考，不要求每次设计变更都同步。
- 工作版本定稿后，必须按 `WORKSPACE_UPDATE_CONFIRMATION.md` 更新 `.archive/`。
