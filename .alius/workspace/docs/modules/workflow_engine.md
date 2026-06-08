# Workflow Engine

更新时间: 2026-06-05 03:43

## 模块职责

Workflow Engine 负责 Plan / Todo / Task Orchestration，将用户目标拆分为可执行步骤，并维护执行状态。

输入:

- user goal
- plan proposal
- todo list
- task graph
- agent/team execution state

输出:

- `PlanNode`
- `TodoItem`
- `TaskState`
- workflow event

## 接口定义

```text
create_plan(goal: str, context: WorkflowContext) -> Result<Plan>
```

```text
update_plan(plan_ref: PlanRef, patch: PlanPatch) -> Result<Plan>
```

```text
run_task(task: TaskSpec) -> Stream<WorkflowEvent>
```

异常:

- plan 为空。
- task graph 有循环依赖。
- 用户拒绝执行计划。

## 内部逻辑

```text
receive goal
-> create or load plan
-> wait for approval if required
-> schedule todo/task nodes
-> emit workflow events
-> update plan state
-> return final workflow result
```

## 数据存储

写入:

- `.alius/memory/episodic/` workflow events。
- `.alius/memory/procedural/` 可复用流程。
- Storage Manager session/task state。

## 异常处理

- 用户修改 plan 时保留历史版本。
- task 失败时标记节点状态，并交给 Budget Manager 统计连续失败。
- agent/team 编排未接入前，不得把 Agent Team 写成可用能力。

## 与其他模块的关系

- 被 Loop Engine 调用。
- 与 Tool Executor 协同执行工具步骤。
- 与 Session Manager 共享 task state。
- 可把稳定流程写入 Procedural Memory。

## 验收标准

- Plan / Todo / Task 状态可观测。
- 用户审批和修改能通过 CoreCommand 回传。
- workflow event 能进入统一 CoreEvent stream。
