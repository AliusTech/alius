# Security And Policy Manager

更新时间: 2026-06-04 22:10

## 模块职责

Security & Policy Manager 负责审批、权限、sandbox 和 allowlist，是本地能力、工具、A2A、MCP、FFI 的统一安全边界。

输入:

- origin
- capability scope
- tool call request
- protocol interface metadata
- project permission config
- shell inspection result

输出:

- `PolicyDecision`
- approval request
- denied reason
- sandbox profile
- shell approval decision

## 接口定义

```text
authorize(request: CapabilityRequest) -> Result<PolicyDecision>
```

```text
request_approval(request: ApprovalRequest) -> Result<ApprovalDecision>
```

```text
resolve_sandbox(origin: Origin, scope: CapabilityScope) -> Result<SandboxProfile>
```

```text
authorize_shell(decision: ShellInspection) -> Result<PolicyDecision>
```

异常:

- origin 未知。
- capability 未声明。
- policy 配置冲突。

## 内部逻辑

```text
receive capability request
-> normalize origin
-> load permissions and allowlist
-> resolve sandbox profile
-> incorporate Shell Gate inspection when request is shell/process/git
-> determine auto-allow / needs approval / deny
-> emit PolicyDecision
```

## 数据存储

读取:

- `.alius/config/permissions.toml`
- `.alius/config/tools.toml`
- `.alius/config/protocol.toml`
- `.alius/config/soul.toml`

写入:

- approval audit trace。
- episodic decision record。

## 默认边界

| Origin | 默认能力 |
| --- | --- |
| LocalTui | 可请求本地工具，但高风险操作需审批 |
| Desktop | JSON-RPC scoped capability |
| IDE | workspace-scoped filesystem |
| RemoteA2A | 最小权限，无本地文件和 shell |
| Embedded SDK | Core Lite 子集 |

## Shell 权限原则

- shell、process、git 必须先经过 Shell Gate 的命令和作用范围检查。
- 作用范围超过 workspace 的读写操作必须授权。
- destructive 命令默认拒绝或强制审批。
- RemoteA2A 和 Embedded SDK 默认没有 shell capability。
- policy 冲突时默认 deny。

## 异常处理

- policy 冲突时默认 deny。
- approval 超时按 deny 处理。
- RemoteA2A 不得继承本地 TUI 的本地权限。
- Shell Gate 返回 unknown scope 时不能自动 allow。

## 与其他模块的关系

- Tool Executor 调用前必须经过本模块。
- Shell Gate 为 shell/process/git 提供细粒度检查结果。
- Protocol Interface Layer 负责传入 origin。
- A2A Adapter 暴露能力受本模块和 Soul Manager 共同约束。
- Budget Manager 可基于 policy 决定是否终止。
- Logging Manager 记录 deny、approval 和 policy conflict。

## 验收标准

- 每个工具调用都有 policy decision。
- 每个协议入口都有默认 capability scope。
- deny 原因可追踪。
- shell/process/git 工具调用包含 Shell Gate decision。
