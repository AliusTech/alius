# Alius 指令完整参考

**版本**: v0.1.0  
**更新日期**: 2026-06-15

---

## 📌 命令分类

Alius 的命令分为两大类：

1. **CLI 命令** - 在终端直接运行的 `alius` 命令
2. **Slash 命令** - 在 TUI Workspace 内使用的 `/` 开头命令

---

## 🖥️ CLI 命令

### 基础命令

#### `alius` 或 `alius repl`
启动交互式 TUI Workspace（默认行为）

```bash
# 基础启动
alius

# 带选项启动
alius --model gpt-4o --provider openai
alius --workspace /path/to/project
alius --config custom-config.toml
alius -v    # 详细日志 (info)
alius -vv   # 调试日志 (debug)
alius -vvv  # 追踪日志 (trace)
```

**全局参数**:
- `-m, --model <MODEL>` - 覆盖默认模型
- `-p, --provider <PROVIDER>` - 覆盖默认提供商
- `--workspace <PATH>` - 设置工作目录
- `-c, --config <PATH>` - 指定配置文件
- `-v, --verbose` - 增加日志详细度

---

#### `alius run`
非交互模式运行单个提示

```bash
# 基础用法
alius run --prompt "解释什么是 Rust 所有权"

# 指定模型
alius run --prompt "生成斐波那契数列" --model claude-3-5-sonnet-20241022
```

**参数**:
- `-p, --prompt <PROMPT>` - 提示文本（必需）
- `-m, --model <MODEL>` - 覆盖默认模型

---

### 配置管理

#### `alius init`
初始化项目级配置（创建 `.alius/config/config.toml`）

```bash
# 交互式向导
alius init
```

**功能**:
- 选择模型服务商（OpenAI、Anthropic、BigModel、DeepSeek、Google）
- 配置 API 密钥
- 选择默认模型
- 选择 Soul 角色
- 设置界面语言

---

#### `alius config`
管理配置设置

```bash
# 显示当前配置
alius config show

# 验证配置文件
alius config validate

# 设置 Soul 角色
alius config soul set <soul-id>

# 列出可用 Soul
alius config soul list
```

**子命令**:
- `show` - 显示当前配置
- `validate` - 验证配置文件语法
- `soul set <ID>` - 激活指定 Soul
- `soul list` - 列出已安装的 Soul

---

### Soul 管理

#### `alius soul`
管理 Soul（角色/提示模板）

```bash
# 更新本地 Soul 缓存
alius soul update

# 列出所有 Soul
alius soul list

# 显示 Soul 详情
alius soul info <soul-id>

# 激活 Soul
alius soul activate <soul-id>

# 查看当前 Soul
alius soul current
```

**子命令**:
- `update` - 从官方仓库同步 Soul
- `list` - 列出已安装的 Soul
- `info <ID>` - 显示 Soul 详细信息
- `activate <ID>` - 激活指定 Soul
- `current` - 显示当前激活的 Soul

---

#### `alius core`
管理官方 Soul 仓库

```bash
# 克隆官方仓库
alius core clone

# 更新官方仓库
alius core update

# 显示仓库路径
alius core path
```

**子命令**:
- `clone` - 克隆 alius-souls 官方仓库
- `update` - 拉取最新的 Soul 更新
- `path` - 显示本地仓库路径

---

### 插件系统

#### `alius plugin`
管理 WASM 插件

```bash
# 列出已安装插件
alius plugin list

# 安装插件
alius plugin install /path/to/plugin

# 显示插件详情
alius plugin info <plugin-id>

# 移除插件
alius plugin remove <plugin-id>
```

**子命令**:
- `list` - 列出已安装的插件
- `install <PATH>` - 从目录安装插件（需要 plugin.toml + plugin.wasm）
- `info <ID>` - 显示插件详细信息
- `remove <ID>` - 移除已安装的插件

---

### MCP 服务器

#### `alius mcp`
管理 MCP（Model Context Protocol）服务器

```bash
# 列出配置的 MCP 服务器
alius mcp list

# 启动 MCP 服务器
alius mcp start <server-name>

# 列出服务器提供的工具
alius mcp tools <server-name>
```

**子命令**:
- `list` - 列出已配置的 MCP 服务器
- `start <NAME>` - 启动指定的 MCP 服务器
- `tools <NAME>` - 列出服务器提供的工具

---

### 工作流

#### `alius workflow`
管理和运行工作流

```bash
# 列出可用工作流
alius workflow list

# 运行工作流
alius workflow run <workflow-name>

# 验证工作流文件
alius workflow validate <workflow.json>
```

**子命令**:
- `list` - 列出可用的工作流
- `run <NAME>` - 运行指定的工作流
- `validate <PATH>` - 验证工作流 JSON 文件

---

### 更新

#### `alius update`
检查和安装 CLI 更新

```bash
# 检查更新
alius update

# 自动安装更新
alius update install
```

---

#### `alius version`
显示版本信息

```bash
alius version
# 输出示例：alius 0.1.0 (git: ed9ccbc)
```

---

## 💬 TUI Workspace Slash 命令

在 TUI Workspace 内（运行 `alius` 后），可以使用以下 slash 命令：

### 帮助与配置

#### `/help`
显示所有可用命令和快捷键

```
/help
```

**显示内容**:
- 所有 slash 命令列表
- 键盘快捷键
- 使用提示

---

#### `/init`
初始化工程配置（TUI 内不可用，需要退出后运行 `alius init`）

---

#### `/config`
启动对话式配置任务

```
/config
```

**配置项**:
- 模型服务商
- 基础 URL
- API 密钥
- 模型
- Soul 角色
- 界面语言

**交互**:
- Enter 确认选择
- Esc 返回上一步
- Tab 切换输入/选择

---

#### `/config show`
显示当前配置

```
/config show
```

---

### 模型管理

#### `/model`
管理模型池

```
/model
```

**功能**:
- 查看当前模型
- 切换模型
- 添加模型到池
- 查看可用模型列表

---

### 会话管理

#### `/session`
会话管理命令

```bash
# 显示当前会话
/session current

# 创建新会话
/session new

# 列出所有会话
/session list

# 加载指定会话
/session load <session-id>

# 清空当前会话
/session clear
```

**子命令**:
- `current` - 显示当前会话信息（ID、模型、消息数）
- `new` - 创建新会话
- `list` - 列出所有已保存的会话
- `load <ID>` - 加载指定会话
- `clear` - 清空当前会话对话历史

---

### 对话历史

#### `/history`
显示对话历史

```
/history
```

**显示**:
- 所有消息（系统、用户、助手、工具）
- 消息角色和内容
- 时间顺序

---

#### `/trace`
显示详细的对话追踪（包含所有消息）

```bash
# 显示所有消息
/trace

# 只显示最新消息
/trace latest
```

---

#### `/clear`
清空对话历史

```
/clear
```

---

### 工具与审查

#### `/tools`
列出当前可用的工具

```
/tools
```

**显示**:
- 内置工具列表
- MCP 工具（如果已连接）
- 插件工具

---

#### `/review`
使用 review_model 审查上一条助手回答

```
/review
```

**功能**:
- 使用专门的审查模型检查输出质量
- 发现潜在错误或改进点
- 提供审查报告

---

#### `/confirm`
切换工具执行确认模式

```bash
# 启用自动确认
/confirm on

# 启用交互式确认
/confirm off

# 查看当前状态
/confirm
```

---

### 记忆系统

#### `/memory`
记忆管理命令

```bash
# 显示所有记忆
/memory show

# 保存新记忆
/memory save <text>

# 列出记忆（同 show）
/memory list

# 清空全局记忆
/memory clear
```

**子命令**:
- `show` / `list` - 显示全局和项目记忆
- `save <TEXT>` - 保存新的记忆条目
- `clear` - 清空全局记忆

---

### 系统诊断

#### `/doctor`
运行系统健康检查

```
/doctor
```

**检查项**:
- ✅ API 密钥配置状态
- ✅ 模型服务商和模型
- ✅ 审查模型配置
- ✅ 当前 Soul 角色
- ✅ 官方 Soul 仓库状态
- ✅ 记忆条目数量
- ✅ MCP 服务器数量
- ✅ 插件数量
- ✅ 工作流数量

---

#### `/mode`
切换旧命令路径的 chat/plan 模式（遗留功能）

```
/mode
```

---

#### `/quit`
退出 TUI Workspace

```
/quit
```

等同于 `Ctrl+C` 或 `Ctrl+D`

---

## ⌨️ 键盘快捷键

在 TUI Workspace 中：

### 全局快捷键
- `Ctrl+C` / `Ctrl+D` - 退出应用
- `Ctrl+Tab` - 切换 Conversation / Agent Team 标签
- `Shift+Tab` - 切换 Plan Mode / Bypass Mode
- `Ctrl+Enter` - 提交输入
- `Esc` - 取消当前提示/输入

### 导航快捷键
- `Tab` - 循环焦点（Input → Conversation → Plans）
- `Up` / `Down` - 在选项中移动
- `Enter` - 确认选项/提交
- `Space` - 多选框切换选中状态

### 滚动快捷键
- 鼠标滚轮 - 滚动面板
- `Shift` + 鼠标选择 - 原生终端文本选择（用于复制）

### 折叠快捷键（新增）
- `Ctrl+O` - 全局展开/折叠对话块
- 鼠标点击折叠块 - 展开/收起单个块

---

## 📁 配置文件

### 全局配置
**位置**: `~/.alius/config/config.toml`

```toml
[runtime]
language = "zh-CN"  # en, zh-CN, ja

[llm]
provider = "anthropic"
base_url = "https://api.anthropic.com"
api_key = "sk-ant-..."
model = "claude-3-5-sonnet-20241022"

[llm.review]
model = "claude-3-5-sonnet-20241022"

[soul]
active = "default"
```

### 项目配置
**位置**: `./.alius/config/config.toml`

优先级高于全局配置，允许项目级覆盖。

### MCP 配置
**位置**: `~/.alius/mcp/servers.toml`

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/allowed/path"]
disabled = false

[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false
```

---

## 🔧 环境变量

Alius 支持以下环境变量：

- `ALIUS_LEGACY_REPL=1` - 使用旧版 rustyline REPL 而非 TUI
- `OPENAI_API_KEY` - OpenAI API 密钥
- `ANTHROPIC_API_KEY` - Anthropic API 密钥
- `BIGMODEL_API_KEY` - BigModel API 密钥
- `DEEPSEEK_API_KEY` - DeepSeek API 密钥
- `GOOGLE_API_KEY` - Google API 密钥

---

## 🎨 TUI 模式

### Plan Mode（计划模式）
- 默认模式
- 用户描述目标 → Alius 生成计划 → 用户批准 → 执行
- 适合复杂任务和多步骤工作

### Bypass Mode（绕过模式）
- 直接执行模式
- 用户输入直接发送给模型，无需计划步骤
- 适合快速对话和简单任务

**切换**: `Shift+Tab`

---

## 📊 日志级别

使用 `-v` 参数控制日志详细度：

- 无参数 - 仅显示警告和错误
- `-v` - Info 级别（基础信息）
- `-vv` - Debug 级别（调试信息）
- `-vvv` - Trace 级别（完整追踪）

---

## 💡 使用示例

### 快速开始
```bash
# 1. 初始化配置
alius init

# 2. 启动 TUI
alius

# 3. 在 TUI 中使用
/help           # 查看帮助
/doctor         # 健康检查
/tools          # 查看可用工具

# 4. 开始对话
描述你的任务...
```

### 非交互模式
```bash
# 单次查询
alius run --prompt "解释 Rust 的生命周期"

# 指定模型
alius run --prompt "生成 Python 排序函数" --model gpt-4o
```

### 高级用法
```bash
# 使用自定义配置
alius --config ./custom-config.toml

# 切换工作目录
alius --workspace /path/to/project

# 详细日志
alius -vv

# 使用旧版 REPL
ALIUS_LEGACY_REPL=1 alius
```

---

## 🐛 故障排查

### 常见问题

**问题**: "尚未配置 LLM 客户端"
```bash
# 解决：运行初始化
alius init
```

**问题**: "API 密钥未设置"
```bash
# 解决：通过 /config 设置 API 密钥
# 或在 TUI 中：
/config
# 选择 "API 密钥" → 输入密钥
```

**问题**: "未找到官方 Soul 仓库"
```bash
# 解决：同步 Soul
alius soul update
```

**问题**: MCP 服务器无法连接
```bash
# 检查配置
alius mcp list

# 查看日志
alius -vv mcp start <server-name>
```

---

## 📚 更多资源

- **配置文档**: `.alius/workspace/docs/modules/config-manager.md`
- **开发文档**: `.alius/workspace/docs/`
- **改进建议**: `.alius/workspace/improvements/`

---

**最后更新**: 2026-06-15  
**维护者**: Alius Team
