# Alius Project Memory

本目录保存项目级三层记忆数据。目标结构:

```text
memory/
  episodic/
  semantic/
  procedural/
  index/
  cache/
  communications/
```

## 三层记忆

| 目录 | 类型 | 说明 |
| --- | --- | --- |
| `episodic/` | 情景记忆 | 对话、turn、事件、工具调用、用户决策 |
| `semantic/` | 语义记忆 | 项目事实、概念、架构知识、文档 chunk、embedding |
| `procedural/` | 程序记忆 | 流程、规则、操作模式、开发规范 |
| `index/` | 检索索引 | 关键词索引、向量元数据、融合检索缓存 |
| `cache/` | 临时缓存 | 可重建缓存，不作为唯一事实来源 |
| `communications/` | 会话记录 | session.json、messages.jsonl、后续 CoreEvent trace |

`project.json` 是 legacy flat memory，后续应按内容迁移到三层记忆。
