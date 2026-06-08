# Compression Worker

更新时间: 2026-06-04 22:10

## 模块职责

Compression Worker 负责后台压缩上下文，并为压缩任务预留 20,000 tokens 空间。

输入:

- `CompressionJob`
- ContextSnapshot
- token budget reservation

输出:

- compressed summary
- source map
- compression trace

## 接口定义

```text
enqueue_compression(job: CompressionJob) -> Result<CompressionJobRef>
```

```text
run_compression(job_ref: CompressionJobRef) -> Stream<CompressionEvent>
```

```text
load_summary(job_ref: CompressionJobRef) -> Result<CompressedContext>
```

异常:

- 预留 token 空间不足。
- compression model 不可用。
- source map 无法重建。

## 内部逻辑

```text
receive trigger from Context Manager
-> reserve 20,000 tokens compression space
-> select compression model
-> summarize older context
-> preserve citations/source map
-> write compressed context
```

## 数据存储

写入:

- `.alius/memory/semantic/` 压缩后的稳定摘要。
- `.alius/memory/episodic/` compression events。
- Storage Manager trace。

## 异常处理

- compression 失败不能破坏原始 session context。
- token 预留失败时返回 Context Manager，由其执行硬截断。
- 压缩摘要必须保留来源引用，不能生成无来源事实。

## 与其他模块的关系

- 由 Context Manager 触发。
- 使用 Model Router / Provider Manager。
- 受 Budget Manager 限制。
- 输出可进入 Memory System。

## 验收标准

- 能按规则预留 20,000 tokens。
- 能异步生成可引用摘要。
- 压缩失败不影响当前 turn 的基本执行。
