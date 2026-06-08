# Prompt Builder

更新时间: 2026-06-05 03:43

## 模块职责

Prompt Builder 负责把 Agent 身份、运行时环境、用户规则、角色模式、工具提示词和后台任务提示词组装为可交给模型的 prompt。

输入:

- `SoulConfig` / `AgentCardView`
- runtime environment snapshot
- user-added rules
- role and mode context
- tool-specific prompt fragments
- background worker prompt fragments

输出:

- `PromptBundle`
- `SystemPrompt`
- `DeveloperPrompt`
- `ToolPromptSet`

## 接口定义

```text
build_prompt(context: PromptBuildContext) -> Result<PromptBundle>
```

```text
render_layer(layer: PromptLayer, context: PromptBuildContext) -> Result<String>
```

```text
validate_prompt_budget(bundle: PromptBundle, budget: TokenBudget) -> Result<PromptBudgetReport>
```

异常:

- 必需身份信息缺失。
- prompt layer 顺序不合法。
- prompt 超出预留 token budget。

## 内部逻辑

```text
load L1 base identity from Soul Manager
-> load L2 runtime environment
-> merge L3 user-added rules
-> apply L4 role and mode additions
-> append L5 tool / background worker prompts
-> validate ordering and token budget
-> return PromptBundle
```

## Prompt 层级

| 层级 | 内容 | 来源 |
| --- | --- | --- |
| L1 | 基础身份 | `soul.toml` / Agent Card-compatible identity |
| L2 | 运行时环境注入 | Runtime / Session / Workspace snapshot |
| L3 | 用户附加规则 | User rules / project policy |
| L4 | 角色和模式附加 | selected role、plan/bypass、frontend/backend 等模式 |
| L5 | 工具 / 后台专用提示词 | Tool Executor、Compression Worker、Workflow Engine |

## 数据存储

Prompt Builder 不直接拥有持久化存储。它读取:

- `.alius/config/soul.toml`
- `.alius/config/config.toml`
- `.alius/memory/procedural/`
- `.alius/workspace/docs/`

## 异常处理

- `soul.toml` 缺失时使用最小默认身份，并报告配置警告。
- 某层 prompt 渲染失败时返回配置错误，不继续调用模型。
- token budget 不足时交给 Context Manager 截断或摘要。

## 与其他模块的关系

- 由 Soul Manager 提供 L1 身份和策略。
- 由 Context Manager 提供上下文窗口约束。
- 输出给 Loop Engine。
- 受 Budget Manager 的 token 限制约束。

## 验收标准

- 能按 L1-L5 顺序生成 prompt。
- 能追踪每段 prompt 的来源。
- 能在超预算时返回结构化错误。
- 工具专用提示词只在工具启用时注入。
