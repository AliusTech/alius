# Retrieval Index

检索索引用于把 episodic、semantic、procedural 三类记忆统一检索。

## 目标文件

```text
.alius/memory/index/retrieval.sqlite
```

## 职责

- keyword index。
- vector metadata。
- score fusion cache。
- memory type routing。
- rerank 结果缓存。

索引可以重建，不应作为唯一事实来源。
