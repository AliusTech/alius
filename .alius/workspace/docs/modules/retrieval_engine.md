# Retrieval Engine

更新时间: 2026-06-04 22:10

## 模块职责

Retrieval Engine 负责关键词和向量融合检索，统一检索 episodic、semantic、procedural 三层记忆。

## 接口定义

```text
hybrid_retrieve(query: str, top_k: int = 5) -> List[Dict]
```

参数:

- `query`: 用户检索关键词。
- `top_k`: 返回结果数量。

返回值:

- 字典列表，每项包含 `content`、`score`、`memory_type`。

异常:

- `query` 为空字符串时抛出 `ValueError`。

## 内部逻辑

```text
query
-> validate query
-> keyword retrieval
-> vector retrieval
-> score fusion
-> rerank
-> top_k results
```

## 数据存储

| 路径 | 说明 |
| --- | --- |
| `.alius/memory/index/retrieval.sqlite` | 关键词索引、融合分数缓存 |
| `.alius/memory/semantic/vectors/` | 向量索引 |
| `.alius/memory/episodic/episodic.sqlite` | 情景记忆来源 |
| `.alius/memory/semantic/semantic.sqlite` | 语义记忆来源 |
| `.alius/memory/procedural/procedural.sqlite` | 程序记忆来源 |

## 异常处理

- query 为空: 返回参数错误。
- vector index 损坏: 自动重建索引。
- semantic memory 不可用: 降级为 keyword retrieval。
- SQLite 锁冲突: 指数退避重试。

## 与其他模块的关系

- 上游: Memory Manager、Context Manager。
- 下游: 三层 memory store。

## 验收标准

- 返回结果包含 `content`、`score`、`memory_type`。
- 任一 memory 层不可用时有明确降级策略。
