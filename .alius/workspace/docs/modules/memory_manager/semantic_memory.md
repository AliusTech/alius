# Semantic Memory

更新时间: 2026-06-04 22:10

## 模块职责

语义记忆保存项目事实、文档 chunk、架构知识和 embedding 元数据。

## 接口定义

```text
upsert_fact(fact: SemanticFact) -> Result<FactRef>
```

```text
index_document(path: Path) -> Result<DocumentIndexReport>
```

```text
semantic_search(query: str, top_k: int) -> Result<Vec<MemoryHit>>
```

## 内部逻辑

```text
document or fact
-> chunk
-> normalize metadata
-> embed if embedding provider available
-> write semantic.sqlite
-> update vectors/
-> update retrieval index
```

## 数据存储

```text
.alius/memory/semantic/semantic.sqlite
.alius/memory/semantic/vectors/
```

核心表:

- `facts`
- `documents`
- `chunks`
- `embeddings`
- `aliases`

## 异常处理

- embedding provider 不可用: 只写 keyword-searchable chunk。
- vector index 损坏: 自动重建。
- 文档不存在: 返回 `DocumentNotFound`。

## 验收标准

- `.alius/workspace/` 文档能被 chunk 并检索。
- semantic 不可用时 retrieval 可降级。
