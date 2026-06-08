# Alius 项目设计文档索引

更新时间: 2026-06-04 22:10
范围: `.alius/memory/design/`

本目录是项目级设计记忆，属于 `.alius/memory/design/`。它用于沉淀当前代码实现、产品约定、运行路径和后续风险，优先反映代码现状，而不是外部 README 中可能滞后的描述。

## 文档地图

1. [项目概览](01-project-overview.md)
   - Alius 的定位、设计目标、workspace 成员和顶层能力。
2. [系统架构](02-architecture.md)
   - Rust workspace 分层、crate 职责、数据流和依赖关系。
3. [运行流](03-runtime-flows.md)
   - `alius init`、交互模式、单次运行、session、review、soul 更新等核心流程。
4. [项目级配置与记忆](04-project-config-and-memory.md)
   - v10 目标 `.alius/config/`、三层 `.alius/memory/`、`.alius/workspace/` 设计文档目录和旧路径迁移规则。
5. [CLI 与交互工作区](05-cli-and-tui.md)
   - Clap 命令、Ratatui workspace、Plan/Bypass、Agent Team 脚手架和旧 REPL。
6. [模型、代理与工具](06-model-agent-tools.md)
   - Provider 抽象、OpenAI/Anthropic 接入、Agent loop、内置工具和权限现状。
7. [扩展系统](07-extension-systems.md)
   - Formula/Soul、MCP、Plugin、Workflow 的职责和接线程度。
8. [发布、文档与风险](08-release-and-documentation-risks.md)
   - npm/Homebrew 发布形态、版本解析、当前实现风险和文档同步事项。
9. [Alius Architecture v10 实现对照文档](09-architecture-v10-implementation-reference.md)
   - 三层架构、Core Runtime、A2A、Core Lite、Cargo Features 和实现检查清单。
10. [协议层优化设计](10-protocol-interface-layer-design.md)
   - Protocol Interface Layer、统一 envelope、CoreRequest/CoreCommand/CoreEvent、传输映射和第一阶段落地计划。

## 当前核心约定

- `alius init` 的 v10 目标行为是创建 `.alius/config/`、`.alius/memory/` 与 `.alius/workspace/`，并写入项目配置、基础记忆目录和模块化设计文档骨架。
- 项目级配置放在 `.alius/`，不是旧的 `alius/`；旧路径只作为兼容读取。
- 新项目配置首选 `.alius/config/config.toml`；`.alius/config.toml` 作为 legacy 路径兼容读取。
- 项目 MCP 配置首选 `.alius/config/mcp.json`；`.alius/mcp.json` 作为 legacy 路径兼容读取。
- 项目 Agent Card 兼容信息首选 `.alius/config/soul.toml`；不再创建项目级 `.alius/soul/` 目录。
- `~/.alius` 是用户级缓存和全局状态，尤其包含 `repos/souls`、`soul/`、`plugins/`、`workflows/`、全局 memory。
- `alius soul update` 负责从 alius-souls 同步所有官方 soul 到 `~/.alius/soul`。
- `alius core update` 是兼容命令，只更新官方 Soul 仓库，不直接同步 soul 缓存。
- 不提供 `alius soul use`；项目 Agent Card 由 `alius init` 或后续配置命令更新。
- `.alius/memory/` 是三层记忆系统目录，目标拆分为 episodic、semantic、procedural；沟通记录放在 `memory/communications/`，系统日志放在 `memory/logs/`。
- `.alius/workspace/` 是项目设计文档目录，包含 `SPEC.md`、非权威 `ROADMAP.md`、`HISTORY.md`、`.archive/`、`docs/` 和 `assets/`。
- `ROADMAP.md` 不作为实现依据，最新实现依据是 `SPEC.md` 和 `docs/` 下的术语、产品、接口、模块、overview 和规范文档。
- workspace 图表主来源是 Markdown Mermaid，不使用外部绘图文件作为维护源。
- 新架构中 CLI / TUI 应通过 Local Rust Interface 进入协议层，再进入 Core Public API；不应直接依赖 Core 内部模块或 provider stream。
- shell/process/git 类命令必须经过 Shell Gate，作用范围不能超过 workspace，除非获得授权。
- runtime、error、exception、audit 日志必须实时记录，并关联 workspace、session、run、trace。

## 推荐目录形态

```text
.alius/
  config/
    config.toml
    soul.toml
    providers.toml
    tools.toml
    permissions.toml
    mcp.json
    protocol.toml
  memory/
    episodic/
    semantic/
    procedural/
    index/
    logs/
      runtime.log.jsonl
      error.log.jsonl
      audit.log.jsonl
      trace/
    communications/
      sessions/
        <session-id>/
          session.json
          messages.jsonl
  workspace/
    SPEC.md
    ROADMAP.md
    HISTORY.md
    .archive/
    docs/
      terms/
        GLOSSARY.md
      products/
        cli.md
        embedded_sdk.md
        desktop_planning.md
        ide_extension_planning.md
        third_party_agent_app.md
      technology/
        TECHNOLOGY_SELECTION.md
      interfaces/
        product_interface_matrix.md
        product_layer.md
        protocol_interface_layer.md
        core_runtime_api.md
      overview/
        ARCH.md
        DATA_FLOW.md
        DIAGRAMS.md
        ENTITY_RELATIONSHIP.md
        ARCHITECTURE_DETAILS.md
      modules/
        config_manager.md
        protocol_interface_layer.md
        core_runtime.md
        session_manager.md
        shell_gate.md
        logging_manager.md
        memory_manager/
          README.md
          episodic_memory.md
          semantic_memory.md
          procedural_memory.md
        workspace_handler.md
        retrieval_engine.md
    assets/
```

## 阅读来源

本批文档整理自当前仓库源码与顶层文档，主要覆盖:

- `Cargo.toml`
- `crates/alius-cli`
- `crates/alius-config`
- `crates/alius-protocol`
- `crates/alius-model`
- `crates/alius-interactive`
- `crates/alius-store`
- `crates/alius-tools`
- `crates/alius-formula`
- `crates/alius-mcp`
- `crates/alius-plugin`
- `crates/alius-workflow`
- `npm-packages`
- `scripts`
- `README*.md`、`CHANGELOG.md`、`CONTRIBUTING.md`
