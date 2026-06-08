# A2A Adapter

更新时间: 2026-06-04 22:10

## 模块职责

A2A Adapter 负责 A2A Server + Client、Task Mapper、Remote Registry，并把远端 task 映射为 Core Request。

输入:

- A2A protocol message
- Agent Card view
- remote task
- remote registry entry
- A2A enable switch

输出:

- `CoreRequest`
- A2A response
- remote agent event
- registry update

## 接口定义

```text
handle_a2a_message(message: A2AMessage) -> Stream<A2AEvent>
```

```text
map_task_to_core_request(task: A2ATask) -> Result<CoreRequest>
```

```text
load_remote_registry(root: Path) -> Result<RemoteRegistry>
```

```text
export_agent_card(config: SoulConfig) -> Result<AgentCardJson>
```

异常:

- A2A 未启用。
- remote agent 不在 allowlist。
- task 无法映射为 CoreRequest。
- Agent Card 发布字段缺失。

## 内部逻辑

```text
check A2A Enable Switch
-> validate remote origin and capability
-> load Agent Card / skills / policy from Soul Manager
-> map A2A task to CoreRequest
-> send request to Session Manager
-> stream CoreEvent back as A2A response
-> update remote registry and trace
```

## 启用策略

| 产品 | 策略 |
| --- | --- |
| CLI | `--a2a` 或 config 可选启用 |
| Desktop | settings toggle，规划能力 |
| Third-party Agent | A2A 标准入口 |
| Embedded SDK | 默认关闭 |

## 数据存储

读取:

- `.alius/config/soul.toml`
- `.alius/config/protocol.toml`
- `.alius/config/permissions.toml`

写入:

- remote registry cache。
- A2A trace。
- episodic remote task events。

## 外部资源

```text
A2A Adapter -> Remote A2A Agents / Apps
```

## 异常处理

- RemoteA2A 默认最小权限，不继承本地权限。
- Agent Card 中未声明的 skill 不得对外暴露。
- A2A 客户端失败不能中断本地 CLI/TUI 主路径。

## 与其他模块的关系

- 接收 A2A Protocol Adapter 输入。
- 由 Soul Manager 提供 Agent Card / policy。
- 调用 Session Manager 进入 Core Runtime。
- 受 Security & Policy Manager 限制。

## 验收标准

- A2A task 能映射为 CoreRequest。
- A2A response 能由 CoreEvent stream 生成。
- CLI/Desktop/Embedded 的启用策略不同。
- 未发布 URL 时不能导出公网 Agent Card。
