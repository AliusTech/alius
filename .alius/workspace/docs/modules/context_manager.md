# Context Manager

更新时间: 2026-06-05 03:43

## 模块职责

Context Manager 负责上下文窗口跟踪、截断、摘要和引用组织。

输入:

- session messages
- retrieved memory
- workspace documents
- tool outputs
- model context limit

输出:

- `ContextSnapshot`
- model-ready context
- compression request
- citation/source map

## 接口定义

```text
build_context(request: ContextBuildRequest) -> Result<ContextSnapshot>
```

```text
truncate_context(snapshot: ContextSnapshot, budget: TokenBudget) -> Result<ContextSnapshot>
```

```text
request_compression(snapshot: ContextSnapshot) -> Result<CompressionJobRef>
```

异常:

- 上下文输入为空且任务需要上下文。
- token 估算失败。
- 引用来源不合法。

## 内部逻辑

```text
load session context
-> merge retrieved memory
-> attach workspace docs and tool outputs
-> estimate tokens
-> reserve compression space
-> truncate or summarize as needed
-> return model-ready context
```

## 数据存储

读取:

- `.alius/memory/episodic/`
- `.alius/memory/semantic/`
- `.alius/workspace/docs/`

写入:

- context snapshot trace。
- compression job metadata。

## 异常处理

- 上下文超限时优先截断低优先级历史，再触发 Compression Worker。
- 引用来源缺失时保留内容但标记 source unknown。
- semantic memory 不可用时保留 session context 和 keyword retrieval。

## 与其他模块的关系

- Loop Engine 调用本模块构建 prompt context。
- Memory System 返回 retrieved context。
- Compression Worker 由本模块触发。
- Budget Manager 管理 token reservation。

## 验收标准

- 能跟踪上下文窗口剩余额度。
- 能组织 memory/document/tool 引用。
- 超限时能触发截断或压缩。
