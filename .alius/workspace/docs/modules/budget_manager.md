# Budget Manager

更新时间: 2026-06-05 03:43

## 模块职责

Budget Manager 负责 token、时间、成本、工具调用次数和连续失败熔断。

图中约束:

- Token Budget 超限终止。
- 连续失败 >= 3 次熔断。
- 管理时间 / 成本 / 工具调用预算。

输入:

- token usage
- tool call count
- elapsed time
- provider cost estimate
- failure event

输出:

- `BudgetDecision`
- termination event
- throttle / fallback recommendation

## 接口定义

```text
check_budget(run_ref: RunRef, usage: UsageSnapshot) -> Result<BudgetDecision>
```

```text
record_failure(run_ref: RunRef, failure: FailureEvent) -> Result<FailureBudgetState>
```

```text
reserve_tokens(run_ref: RunRef, amount: TokenCount, reason: str) -> Result<TokenReservation>
```

异常:

- budget policy 缺失。
- usage counter 不一致。
- reservation 超过剩余额度。

## 内部逻辑

```text
receive usage update
-> compare token / time / cost / tool-call limits
-> update failure counter
-> if token exceeded, terminate
-> if consecutive failures >= 3, circuit break
-> emit runtime/audit log
-> emit BudgetDecision
```

## 数据存储

写入:

- run-level usage snapshot。
- failure counters。
- termination reason。
- budget runtime/audit log。

可存放于:

- `.alius/memory/episodic/`
- Storage Manager trace。

## 异常处理

- usage 无法读取时默认保守限制。
- 失败计数只按同一 run_ref 或明确同一 task scope 累计。
- 熔断后 Loop Engine 不得继续自动工具循环。
- 日志不可用时仍必须返回 BudgetDecision。

## 与其他模块的关系

- Loop Engine 每轮执行调用本模块。
- Model Router 使用成本预算决定路由。
- Tool Executor 上报工具调用数量和失败。
- Context Manager 请求 token reservation。
- Logging Manager 记录预算超限、熔断和保守降级原因。

## 验收标准

- token 超限会产生终止事件。
- 连续失败达到 3 次会熔断。
- budget decision 能进入 CoreEvent stream。
- budget 终止和熔断可在日志中查询。
