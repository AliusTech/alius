# Shell Gate

更新时间: 2026-06-04 22:10

## 模块职责

Shell Gate 是所有 shell、process、git、脚本执行类能力的强制门禁。它在 Tool Executor 调用本地 OS 能力之前检查命令、参数、目标路径和作用范围。

输入:

- `ShellCommandRequest`
- `Origin`
- `CapabilityScope`
- `WorkspaceRef`
- `PermissionConfig`
- `ApprovalDecision`

输出:

- `ShellGateDecision`
- normalized command plan
- approval request
- denied reason

## 接口定义

```text
inspect_command(request: ShellCommandRequest) -> Result<ShellInspection>
```

参数:

- `request.command`: 原始命令。
- `request.args`: 参数数组。
- `request.cwd`: 命令工作目录。
- `request.origin`: 请求来源。
- `request.workspace_root`: 当前 workspace 根目录。

返回:

- 解析后的命令、风险等级、目标路径、是否越权、是否 destructive。

异常:

- 命令无法解析。
- cwd 不存在。

```text
authorize_shell(request: ShellCommandRequest) -> Result<ShellGateDecision>
```

返回:

- `Allow`
- `Deny`
- `ApprovalRequired`

异常:

- origin 未声明 shell capability。
- policy 配置冲突。

```text
normalize_scope(request: ShellCommandRequest) -> Result<ScopeAnalysis>
```

返回:

- 读取路径集合。
- 写入路径集合。
- 删除路径集合。
- 是否超过 workspace。

## 内部逻辑

```text
receive shell command
-> parse command and args
-> classify command risk
-> resolve cwd and path arguments
-> detect read/write/delete scope
-> compare scope with workspace root
-> apply hard deny rules
-> apply permission allowlist
-> request approval if needed
-> return ShellGateDecision
```

## 风险等级

| 等级 | 示例 | 默认策略 |
| --- | --- | --- |
| Low | `ls`, `pwd`, `rg`, `git status` | workspace 内可允许 |
| Medium | `git diff`, `npm test`, `cargo test`, `mkdir` | workspace 内按配置允许 |
| High | `git commit`, `git push`, `chmod`, `mv`, `cp` 写入 | 需要上下文判断或审批 |
| Critical | `rm -rf`, `sudo`, `dd`, destructive recursive delete | 默认拒绝或强制审批 |

## 硬性拒绝规则

- 不允许执行会删除根目录、home、workspace 根本身或不明确路径的大范围删除命令。
- `rm -rf /`、`rm -rf ~`、`rm -rf .`、`rm -rf *`、`rm -rf <workspace_root>` 默认拒绝。
- 不允许未经授权读写 workspace 外路径。
- 不允许 RemoteA2A 和 EmbeddedSdk 请求本地 shell。
- 不允许通过 `sh -c`、`bash -c`、管道、重定向、通配符绕过路径分析。
- 不允许命令把输出重定向到 workspace 外文件。

## 作用范围检测

Shell Gate 必须识别:

- cwd 是否在 workspace 内。
- 参数中的相对路径和绝对路径。
- glob 展开后的实际路径集合。
- `..` 是否越过 workspace。
- symlink 是否指向 workspace 外。
- 重定向目标。
- 删除、移动、复制、写入、chmod、chown 等副作用。

当路径分析无法确定时，策略为:

```text
unknown scope -> ApprovalRequired or Deny
```

## 数据存储

读取:

- `.alius/config/permissions.toml`
- `.alius/config/tools.toml`
- `.alius/config/protocol.toml`

写入:

- shell inspection trace。
- approval audit。
- runtime log。
- episodic decision record。

## 异常处理

- 命令解析失败: `Deny`，并记录 `ShellParseError`。
- 路径 canonicalize 失败: `ApprovalRequired` 或 `Deny`。
- symlink 指向 workspace 外: 需要授权。
- approval 超时: `Deny`。
- 执行后返回非零: 记录 runtime log 和 tool failure。

## 与其他模块的关系

- Tool Executor 调用 Shell Gate 后才能执行 shell/process/git。
- Security & Policy Manager 提供 origin、capability 和审批策略。
- Logging Manager 记录命令检查、拒绝、审批和执行错误。
- Budget Manager 记录命令调用次数和失败次数。
- Storage Manager 持久化 trace 和 audit。

## 验收标准

- 所有 shell/process/git 工具调用都经过 Shell Gate。
- workspace 外读写必须触发审批或拒绝。
- `rm -rf` 类高风险删除有硬性拒绝测试。
- symlink、glob、重定向和 `bash -c` 有覆盖测试。
- 每次 decision 可按 trace_id 查询。
