# Semantic Memory

语义记忆保存项目事实、架构知识、文档 chunk 和 embedding 元数据。

## 目标文件

```text
.alius/memory/semantic/semantic.sqlite
.alius/memory/semantic/vectors/
```

## 核心表

| 表 | 说明 |
| --- | --- |
| `facts` | 稳定项目事实 |
| `documents` | 文档来源记录 |
| `chunks` | 可检索文档 chunk |
| `embeddings` | embedding 元数据和向量引用 |
| `aliases` | 术语、模块、路径别名 |

## 写入来源

- `.alius/workspace/` 模块文档。
- 代码分析产出的稳定事实。
- 用户显式保存的项目知识。
- 从 legacy `.alius/memory/project.json` 迁移来的普通事实。
