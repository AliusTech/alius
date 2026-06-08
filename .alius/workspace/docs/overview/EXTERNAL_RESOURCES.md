# External Resources

更新时间: 2026-06-04 22:10

## 定位

External Resources 不是 Alius 的架构层，而是 Core Runtime 调用的外部依赖。所有外部调用必须由 Core 模块发起，并经过 Security & Policy Manager 的能力边界约束。

## 资源清单

| ID | 资源 | 调用方 | 用途 | 约束 |
| --- | --- | --- | --- | --- |
| `x_model` | Model Provider APIs | Provider Manager、Embedded Core Lite | 模型推理、流式输出、远程 inference | 必须经过 Model Router / Provider Manager 或 Core Lite 子集 |
| `x_embed` | Embedding APIs / Memory Gateway | Memory System、Embedded Core Lite | embedding、向量索引、远程 memory gateway | semantic 不可用时必须可降级 |
| `x_local` | Local OS Capabilities | Tool Executor / Shell Gate / Security Policy | File / Process / Shell / Git | shell/process/git 必须经过 Shell Gate；workspace 外读写必须授权 |
| `x_mcp` | External MCP Servers | Tool Executor | MCP 工具、MCP Resource | 必须经工具注册和权限策略 |
| `x_a2a` | Remote A2A Agents / Apps | A2A Adapter、第三方 Agent 应用 | 外部 A2A 对端通信 | 必须按 Agent Card skills / capabilities / policy 暴露能力 |
| `x_soulrepo` | Official Soul Repository | Installed Soul Data | AliusTech/alius-souls 同步或更新 | 只能作为 legacy soul 导入或全局缓存来源 |

## 外部调用连线

```text
Provider Manager -> Model Provider APIs
Memory System -> Embedding APIs / Memory Gateway
Tool Executor -> Shell Gate -> Security Policy -> Local OS Capabilities
Tool Executor -> External MCP Servers
A2A Adapter -> Remote A2A Agents / Apps
Third-party Agent App -> Remote A2A Agents / Apps
Installed Soul Data -> Official Soul Repository
Embedded Core Lite -> Model Provider APIs
Embedded Core Lite -> Embedding APIs / Memory Gateway
```

## 安全要求

- 外部网络、文件系统、shell、git、MCP 和 A2A 调用必须保留 origin、capability scope、trace id。
- RemoteA2A 默认能力最小化，不拥有本地文件和 shell 权限。
- Embedded SDK 默认关闭 A2A，并只能使用远程模型或远程 memory gateway。
- Tool Executor 不直接绕过 Security & Policy Manager。
- Tool Executor 不能直接越权执行 shell/process/git，本地命令必须经过 Shell Gate。
- 读写超过 workspace 的路径必须审批；无法判断作用范围时不能自动允许。
