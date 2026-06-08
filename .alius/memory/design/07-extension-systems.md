# 07. 扩展系统

更新时间: 2026-06-04 22:10

## Formula 与 Soul

Formula 系统由 `alius-formula` 管理。

### 官方仓库

默认路径:

```text
~/.alius/repos/souls
```

远程:

```text
git@github.com:AliusTech/alius-souls.git
https://github.com/AliusTech/alius-souls.git
```

更新策略:

- 已存在 `.git` 时执行 `git fetch --all` 和 `git reset --hard origin/main`。
- 不存在时先 SSH clone，失败后 HTTPS clone。

### Formula 解析

读取:

```text
Formula/souls/*.toml
```

结构:

- id
- name
- version
- type
- description
- license
- model preferences

### Soul 安装

安装路径:

```text
~/.alius/soul/<id>/versions/<version>/
  formula.toml
  prompts/
    identity.md
    style.md
    rules.md
```

`load_soul_prompts(id)` 会选择 versions 中排序后的 latest version，并按顺序拼接:

1. identity.md
2. style.md
3. rules.md

### Legacy Soul 到 Agent Card

项目 Agent Card 文件:

```text
.alius/config/soul.toml
```

设计约束:

- 安装是全局缓存行为。
- Agent Card 是项目级配置行为。
- 项目级不创建单独的 soul 目录。
- 不能用 `alius soul use`。
- `alius init` 应直接生成或选择 Agent Card。
- `alius config soul --role <id>` 只作为 legacy soul 导入 Agent Card 的兼容入口。

## MCP

`alius-mcp` 是 MCP stdio client。

配置格式:

```json
{
  "servers": {
    "name": {
      "command": "cmd",
      "args": [],
      "env": {}
    }
  }
}
```

当前命令:

- `alius mcp list`
- `alius mcp start <name>`
- `alius mcp tools <name>`

当前协议:

- initialize
- notifications/initialized
- tools/list
- tools/call

当前限制:

- CLI 只做 server 管理和 tool 列举。
- MCP tools 尚未合并进 `ToolRegistry`。
- MCP config 查找没有向上搜索项目 `.alius`。
- MCP server 生命周期是命令级启动和停止，不是 workspace 长驻连接。

设计方向:

- 使用 `.alius/config/mcp.json` 作为项目级 MCP 声明；`.alius/mcp.json` 只作为 legacy 兼容读取。
- 将 MCP tools 适配为 `AliusTool` 或 provider-agnostic `ToolDef`。
- 将 MCP call 纳入 agent event 和 permission/confirmation 体系。

## WASM Plugin

`alius-plugin` 管理全局 WASM 插件。

安装路径:

```text
~/.alius/plugins/<plugin-id>/
  plugin.toml
  plugin.wasm
  schemas/
```

manifest:

- id
- name
- version
- description
- author

当前 ABI 设想:

```text
alius_plugin_list_tools() -> JSON array
alius_plugin_call_tool(name, args_json) -> JSON result
```

当前实现状态:

- 支持从本地目录安装 plugin。
- 支持 list/info/remove。
- 支持 `call_plugin_tool()` 加载 WASM 并调用导出函数。
- 未接入 CLI 工具注册和 agent loop。

当前风险:

- ABI 使用固定 memory offset，容易和真实插件内存管理冲突。
- 缺少 WASI、capability sandbox、schema 注册、版本兼容策略。

设计方向:

- 先把 plugin 的 tools/list 和 schemas 纳入 ToolRegistry。
- 明确 WASM ABI 约定和内存分配协议。
- 以权限配置控制插件能力。

## Workflow

`alius-workflow` 是 JSON workflow 解释器。

workflow:

```json
{
  "name": "name",
  "description": "description",
  "steps": []
}
```

step type:

- prompt
- tool
- http
- condition

变量插值:

```text
{{step_id.output}}
```

当前执行状态:

- prompt step 只返回 `[prompt] <interpolated>`，没有调用 LLM。
- tool step 只返回 `[tool:name] args=...`，没有调用 ToolRegistry。
- http step 已真实发 HTTP 请求。
- condition step 是占位逻辑。

设计方向:

- workflow 应复用 `LlmClient`、`ToolRegistry`、`PermissionManager`。
- workflow execution context 应和 session/trace 关联。
- project workflow 可考虑迁移到 `.alius/workflows/`，当前实现只使用 `~/.alius/workflows/`。

## 扩展系统边界

| 系统 | 当前成熟度 | 是否接入默认 workspace |
| --- | --- | --- |
| Agent Card | 目标接入项目 init、system prompt 和 A2A discovery | 规划中 |
| MCP | CLI 管理可用 | 否 |
| Plugin | 安装/列举/调用函数雏形 | 否 |
| Workflow | JSON 解析和部分执行 | 否 |

优先级建议:

1. 完善 Agent Card 和 project memory，因为它们是 v10 主路径输入。
2. 接入 Agent tool loop，让内置工具先跑通。
3. 再把 MCP tools 纳入工具注册。
4. 最后收敛 Plugin ABI 和 Workflow runtime。
