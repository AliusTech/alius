# Provider Manager

更新时间: 2026-06-05 03:43

## 模块职责

Provider Manager 负责对接模型供应商 API，隐藏 OpenAI、Anthropic、Google、BigModel、Custom 等供应商差异。

输入:

- `ProviderRequest`
- provider credentials
- provider endpoint
- streaming options

输出:

- model response stream
- usage report
- provider error

## 接口定义

```text
send_chat(request: ProviderRequest) -> Stream<ProviderEvent>
```

```text
list_models(provider: ProviderId) -> Result<Vec<ModelInfo>>
```

```text
validate_credentials(provider: ProviderId) -> Result<CredentialStatus>
```

异常:

- API key 缺失或无效。
- provider endpoint 不可达。
- 模型不存在或不支持当前模式。

## 内部逻辑

```text
receive ProviderRequest
-> resolve provider endpoint and auth
-> map Alius message format to provider format
-> send request
-> normalize stream delta / usage / errors
-> emit provider runtime/error log
-> return ProviderEvent stream
```

## 数据存储

读取:

- `.alius/config/providers.toml`
- user keychain / environment variables

不保存模型响应正文；响应由 Storage Manager / Episodic Memory 根据策略保存。

## 外部资源

```text
Provider Manager -> Model Provider APIs
```

## 异常处理

- transient network error 可按 policy 重试。
- rate limit 返回可恢复错误，交给 Budget Manager 或 Loop Engine 决定是否等待/降级。
- provider-specific error 必须归一化为 `ProviderError`。
- provider 错误必须写入 error log，且不得泄露 API key。

## 与其他模块的关系

- 被 Model Router 调用。
- 事件返回 Loop Engine。
- usage 上报 Budget Manager。
- credentials 读取依赖 Config Manager / Storage Manager。
- runtime/error 日志写入 Logging Manager。

## 验收标准

- 支持 OpenAI、Anthropic、Google、BigModel、Custom 的配置表达。
- 流式输出归一化为统一事件。
- provider 错误不会泄漏不稳定的供应商私有结构给上层。
- provider 错误日志已脱敏。
