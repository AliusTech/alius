# Model Router

更新时间: 2026-06-05 03:43

## 模块职责

Model Router 负责三层模型分流，将任务按复杂度、成本、上下文长度和策略路由到 light / medium / high 模型档位。

输入:

- `ModelRequest`
- task complexity signal
- budget policy
- provider availability
- runtime profile

输出:

- `ResolvedModelRoute`
- `ProviderRequest`
- fallback route

## 接口定义

```text
route_model(request: ModelRequest) -> Result<ResolvedModelRoute>
```

```text
fallback_route(error: ProviderError, previous: ResolvedModelRoute) -> Result<ResolvedModelRoute>
```

```text
estimate_route_cost(route: ResolvedModelRoute, tokens: TokenEstimate) -> Result<CostEstimate>
```

异常:

- 没有可用 provider。
- 指定模型不可用。
- budget policy 禁止高成本模型。

## 内部逻辑

```text
classify task as light / medium / high
-> apply user / project model preference
-> apply budget limits
-> resolve provider alias
-> check provider availability
-> return ProviderRequest
```

## 数据存储

读取:

- `.alius/config/providers.toml`
- `.alius/config/config.toml`
- `.alius/config/permissions.toml`

不直接持久化。

## 异常处理

- high 模型不可用时可降级到 medium，但必须在 event stream 中记录。
- budget 不允许降级以外的替代方案时返回 `BudgetExceeded`。
- provider credentials 缺失时返回配置错误。

## 与其他模块的关系

- 由 Loop Engine 调用。
- 调用 Provider Manager。
- 受 Budget Manager 约束。
- Embedded Core Lite 只使用其远程调用子集。

## 验收标准

- 能按 light / medium / high 生成路由。
- 能根据 provider 配置解析模型别名。
- 能在失败时输出明确 fallback 原因。
