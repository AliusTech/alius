# Memory Manager

更新时间: 2026-06-05 03:43

## 模块职责

Memory Manager 负责三层记忆的写入路由、读取协调和与 Retrieval Engine 的集成。三层记忆不是对话日志目录，而是面向工程工作区的长期上下文系统。

输入:

- CoreEvent。
- session / turn / event metadata。
- session message summary or reference。
- 用户显式保存的记忆。
- documents chunk。

输出:

- episodic memory writes。
- semantic memory writes。
- procedural memory writes。
- retrieval query request。

## 接口定义

```text
write_memory(item: MemoryItem) -> Result<MemoryRef>
```

```text
classify_memory(item: MemoryItem) -> MemoryType
```

```text
retrieve(query: RetrievalQuery) -> Result<Vec<MemoryHit>>
```

```text
apply_retention_policy(item: MemoryItem) -> Result<MemoryItem>
```

## 内部逻辑

```text
MemoryItem
-> classify episodic / semantic / procedural
-> apply retention and privacy policy
-> validate payload
-> write to corresponding store
-> update retrieval index
```

## 三层记忆定义

| 记忆层 | 保存对象 | 不保存对象 |
| --- | --- | --- |
| Episodic | session、turn、CoreEvent、工具调用、用户决策、时间线摘要 | 不默认保存完整聊天原文 |
| Semantic | 稳定项目事实、架构决策、文档 chunk、术语定义、embedding 元数据 | 不保存一次性事件时间线 |
| Procedural | 可复用流程、规则、playbook、失败模式、验证命令 | 不保存普通事实和临时对话 |

用户输入处理原则:

- 用户显式保存的记忆可进入 semantic 或 procedural。
- 普通对话输入默认进入 episodic 的 turn 记录，但是否保存原文由 retention policy 决定。
- 检索索引只包含被标记为 searchable 的内容。
- 敏感信息必须脱敏或拒绝保存。

## 数据存储

- `.alius/memory/episodic/episodic.sqlite`
- `.alius/memory/semantic/semantic.sqlite`
- `.alius/memory/procedural/procedural.sqlite`
- `.alius/memory/index/retrieval.sqlite`
- `.alius/memory/logs/`

## 异常处理

- 单层 memory 不可用: 该层写入失败，但其他层不受影响。
- index 损坏: 标记重建并继续读取源数据。
- retention policy 拒绝保存原文: 保存摘要和引用。
- sensitive content detected: 脱敏后保存或跳过 searchable index。

## 与其他模块的关系

- 上游: Loop Engine、Workspace Handler、Session Manager。
- 下游: Retrieval Engine、Context Manager、Logging Manager。

## 验收标准

- 新写入内容能按类型进入对应 memory。
- legacy `project.json` 可被分类迁移。
- 情景记忆可按 session/run/trace 重建事件时间线。
- 用户输入保存策略可配置和测试。
