# Entity Relationship Diagrams

更新时间: 2026-06-04 22:10

## 定位

本文件使用 Markdown Mermaid ER 图描述 Alius v10 的核心实体关系。它用于补充架构图和数据流图，帮助实现阶段对齐存储模型、配置模型和运行时对象。

## Runtime Entities

```mermaid
erDiagram
  PROJECT ||--|| PROJECT_CONFIG : owns
  PROJECT ||--|| SOUL_CONFIG : owns
  PROJECT ||--o{ SESSION : contains
  SESSION ||--o{ TURN : contains
  TURN ||--o{ MESSAGE : records
  TURN ||--o{ CORE_EVENT : emits
  TURN ||--o{ TOOL_CALL : requests
  TURN ||--o{ TRACE_EVENT : traces
  TURN ||--o{ LOG_RECORD : logs
  TURN ||--o{ BUDGET_SNAPSHOT : updates

  SESSION ||--o{ LOG_RECORD : has
  TRACE_EVENT ||--o{ LOG_RECORD : correlates
  SOUL_CONFIG ||--o{ SKILL : exposes
  SOUL_CONFIG ||--o{ SUPPORTED_INTERFACE : publishes
  SOUL_CONFIG ||--|| AGENT_CARD_VIEW : normalizes_to

  PROJECT_CONFIG ||--|| PROVIDER_CONFIG : includes
  PROJECT_CONFIG ||--|| TOOL_CONFIG : includes
  PROJECT_CONFIG ||--|| PERMISSION_CONFIG : includes
  PROJECT_CONFIG ||--|| PROTOCOL_CONFIG : includes
  PROJECT_CONFIG ||--|| MCP_CONFIG : includes

  TOOL_CALL }o--|| TOOL_CONFIG : uses
  TOOL_CALL }o--|| PERMISSION_CONFIG : authorized_by
  TOOL_CALL }o--o| SHELL_INSPECTION : gated_by
  SHELL_INSPECTION }o--|| PERMISSION_CONFIG : checked_against
  CORE_EVENT }o--|| TRACE_EVENT : materializes
  CORE_EVENT }o--o| LOG_RECORD : emits
```

## Memory Entities

```mermaid
erDiagram
  MEMORY_ITEM ||--o| EPISODIC_MEMORY : classified_as
  MEMORY_ITEM ||--o| SEMANTIC_MEMORY : classified_as
  MEMORY_ITEM ||--o| PROCEDURAL_MEMORY : classified_as

  EPISODIC_MEMORY ||--o{ SESSION : references
  EPISODIC_MEMORY ||--o{ TURN : references
  EPISODIC_MEMORY ||--o{ MESSAGE : references
  EPISODIC_MEMORY ||--o{ CORE_EVENT : references

  SEMANTIC_MEMORY ||--o{ DOCUMENT_CHUNK : indexes
  SEMANTIC_MEMORY ||--o{ FACT : stores
  SEMANTIC_MEMORY ||--o{ VECTOR_EMBEDDING : embeds

  PROCEDURAL_MEMORY ||--o{ PROCEDURE : stores
  PROCEDURAL_MEMORY ||--o{ RULE : stores
  PROCEDURAL_MEMORY ||--o{ PLAYBOOK : stores
  PROCEDURAL_MEMORY ||--o{ FAILURE_PATTERN : stores

  RETRIEVAL_INDEX ||--o{ MEMORY_ITEM : indexes
  RETRIEVAL_INDEX ||--o{ VECTOR_EMBEDDING : references
```

## Workspace Document Entities

```mermaid
erDiagram
  WORKSPACE ||--|| SPEC : owns
  WORKSPACE ||--|| ROADMAP : references
  WORKSPACE ||--|| HISTORY : owns
  WORKSPACE ||--o{ OVERVIEW_DOCUMENT : contains
  WORKSPACE ||--o{ MODULE_DOCUMENT : contains
  WORKSPACE ||--o{ STANDARD_DOCUMENT : contains
  WORKSPACE ||--o{ PRODUCT_DOCUMENT : contains
  WORKSPACE ||--o{ INTERFACE_DOCUMENT : contains
  WORKSPACE ||--o{ TECHNOLOGY_DOCUMENT : contains
  WORKSPACE ||--|| TERMINOLOGY_DOCUMENT : owns
  WORKSPACE ||--o{ ASSET : contains
  WORKSPACE ||--|| ARCHIVE_SNAPSHOT : compares_with

  MODULE_DOCUMENT ||--|| MODULE_IMPLEMENTATION : specifies
  PRODUCT_DOCUMENT ||--o{ INTERFACE_DOCUMENT : maps_to
  TERMINOLOGY_DOCUMENT ||--o{ TERM : defines
  OVERVIEW_DOCUMENT ||--o{ MERMAID_DIAGRAM : contains
  STANDARD_DOCUMENT ||--o{ WORKFLOW_RULE : defines
  HISTORY ||--o{ HISTORY_ENTRY : records
  ARCHIVE_SNAPSHOT ||--o{ ARCHIVED_DOCUMENT : stores
```
