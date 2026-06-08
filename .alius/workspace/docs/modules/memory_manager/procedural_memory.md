# Procedural Memory

更新时间: 2026-06-04 22:10

## 模块职责

程序记忆保存流程、规则、操作模式和可复用 playbook。

## 接口定义

```text
upsert_procedure(procedure: Procedure) -> Result<ProcedureRef>
```

```text
match_procedure(context: TaskContext) -> Result<Vec<ProcedureHit>>
```

```text
record_failure_pattern(pattern: FailurePattern) -> Result<PatternRef>
```

## 内部逻辑

```text
task or validation experience
-> extract stable rule or procedure
-> validate scope
-> write procedural.sqlite
-> update retrieval index
```

## 数据存储

```text
.alius/memory/procedural/procedural.sqlite
```

核心表:

- `procedures`
- `rules`
- `playbooks`
- `validation_commands`
- `failure_patterns`

## 异常处理

- procedure 与现有规则冲突: 记录冲突并等待人工确认。
- 规则缺少适用范围: 拒绝写入。

## 验收标准

- 常用验证命令和失败修复模式能被检索。
- 规则有明确适用范围。
