# 如何添加 MCP 服务器 - 完整指南

**版本**: v0.1.1-hotfix  
**更新日期**: 2026-06-17

---

## 🎯 快速添加（3 步）

### 1. 创建配置文件

```bash
# 创建 MCP 配置目录
mkdir -p ~/.alius/mcp

# 创建配置文件
touch ~/.alius/mcp/servers.toml
```

### 2. 编辑配置文件

```bash
# 使用编辑器打开
vim ~/.alius/mcp/servers.toml
# 或
nano ~/.alius/mcp/servers.toml
# 或
open -e ~/.alius/mcp/servers.toml
```

### 3. 添加服务器配置

```toml
# 基础示例 - Filesystem 服务器
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
disabled = false
```

---

## 📝 详细步骤

### 步骤 1: 确认配置文件位置

MCP 配置文件应该位于：
```
~/.alius/mcp/servers.toml
```

### 步骤 2: 创建配置文件（如果不存在）

```bash
# 一键创建配置文件和示例
cat > ~/.alius/mcp/servers.toml << 'EOF'
# MCP 服务器配置
# 格式: [servers.<服务器名称>]

# Filesystem 服务器 - 文件系统操作
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp", "/Users/$USER/Documents"]
disabled = false

# GitHub 服务器 - GitHub 操作
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "your-github-token"
EOF
```

### 步骤 3: 验证配置

```bash
# 检查配置文件
cat ~/.alius/mcp/servers.toml

# 测试 MCP
alius mcp list
```

---

## 🌟 常见 MCP 服务器添加示例

### 1. Filesystem（文件系统）

**功能**: 读写文件、列出目录

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allow"]
disabled = false
```

**多路径示例**:
```toml
[servers.filesystem]
command = "npx"
args = [
    "-y", 
    "@modelcontextprotocol/server-filesystem",
    "/tmp",
    "/Users/username/Documents",
    "/Users/username/Projects"
]
disabled = false
```

### 2. GitHub

**功能**: 创建 Issue、PR、查看仓库

```toml
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_token_here"
```

**获取 GitHub Token**:
1. 访问 https://github.com/settings/tokens
2. 点击 "Generate new token (classic)"
3. 选择权限: `repo`, `read:org`
4. 复制 token 到配置

### 3. PostgreSQL（数据库）

**功能**: 执行 SQL 查询

```toml
[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = false

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://user:password@localhost:5432/database"
```

### 4. Slack

**功能**: 发送消息、列出频道

```toml
[servers.slack]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-slack"]
disabled = false

[servers.slack.env]
SLACK_BOT_TOKEN = "xoxb-your-token"
```

### 5. Google Drive

**功能**: 读写 Google Drive 文件

```toml
[servers.gdrive]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-gdrive"]
disabled = false

[servers.gdrive.env]
GOOGLE_CLIENT_ID = "your-client-id"
GOOGLE_CLIENT_SECRET = "your-client-secret"
```

### 6. Brave Search

**功能**: 网络搜索

```toml
[servers.brave-search]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-brave-search"]
disabled = false

[servers.brave-search.env]
BRAVE_API_KEY = "your-api-key"
```

---

## 🔧 自定义本地服务器

### 方法 1: Node.js 脚本

```toml
[servers.my-server]
command = "node"
args = ["/path/to/my-server.js"]
disabled = false
```

### 方法 2: Python 脚本

```toml
[servers.python-server]
command = "python3"
args = ["/path/to/server.py"]
disabled = false
```

### 方法 3: 二进制文件

```toml
[servers.custom-binary]
command = "/usr/local/bin/my-mcp-server"
args = ["--config", "/path/to/config.json"]
disabled = false
```

---

## ⚙️ 配置选项详解

### 基础配置

```toml
[servers.<名称>]
command = "<可执行命令>"          # 必需
args = ["<参数1>", "<参数2>"]    # 必需
disabled = false                  # 可选，默认 false
```

### 环境变量

```toml
[servers.<名称>.env]
API_KEY = "your-key"
API_URL = "https://api.example.com"
DEBUG = "true"
```

### 完整示例

```toml
[servers.my-api]
command = "npx"
args = ["-y", "@example/mcp-server"]
disabled = false

[servers.my-api.env]
API_KEY = "secret-key"
API_ENDPOINT = "https://api.example.com"
TIMEOUT = "30000"
```

---

## ✅ 验证配置

### 1. 检查配置文件语法

```bash
# 查看配置
cat ~/.alius/mcp/servers.toml

# 检查语法（确保是有效的 TOML）
# 可以使用在线工具: https://www.toml-lint.com/
```

### 2. 列出配置的服务器

```bash
alius mcp list

# 输出示例:
# Available MCP servers:
#   filesystem (enabled)
#   github (enabled)
```

### 3. 测试服务器启动

```bash
# 启动单个服务器
alius mcp start filesystem

# 输出示例:
# Starting MCP server: filesystem
# Server started successfully
```

### 4. 查看可用工具

```bash
# 列出服务器的工具
alius mcp tools filesystem

# 输出示例:
# Available tools from filesystem:
#   read_file - Read file content
#   write_file - Write content to file
#   list_directory - List directory contents
```

---

## 🔍 故障排除

### 问题 1: 配置文件不存在

**错误**:
```
No MCP servers configured.
```

**解决**:
```bash
# 检查文件是否存在
ls -la ~/.alius/mcp/servers.toml

# 如果不存在，创建它
mkdir -p ~/.alius/mcp
touch ~/.alius/mcp/servers.toml
```

### 问题 2: TOML 语法错误

**错误**:
```
Failed to parse config: ...
```

**解决**:
```bash
# 检查语法
cat ~/.alius/mcp/servers.toml

# 常见错误:
# - 缺少引号
# - 方括号不匹配
# - 使用了错误的字符

# 使用正确的格式
[servers.name]
command = "npx"  # 需要引号
args = ["arg1", "arg2"]  # 数组格式
```

### 问题 3: Node.js 未安装

**错误**:
```
command not found: npx
```

**解决**:
```bash
# 安装 Node.js
# macOS:
brew install node

# 或下载: https://nodejs.org/

# 验证
node --version
npx --version
```

### 问题 4: 权限错误

**错误**:
```
Permission denied: /path/to/file
```

**解决**:
```bash
# 检查路径权限
ls -ld /path/to/file

# 确保 Alius 可以访问该路径
# 只配置你有权限的目录

[servers.filesystem]
args = ["-y", "@modelcontextprotocol/server-filesystem", 
        "/tmp",  # 通常可访问
        "$HOME/Documents"  # 你的文档目录
]
```

---

## 📋 配置模板

### 完整配置模板

```toml
# ~/.alius/mcp/servers.toml

# ========================================
# Filesystem 服务器
# ========================================
[servers.filesystem]
command = "npx"
args = [
    "-y",
    "@modelcontextprotocol/server-filesystem",
    "/tmp",
    "/Users/username/Documents",
    "/Users/username/Projects"
]
disabled = false

# ========================================
# GitHub 服务器
# ========================================
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_token_here"

# ========================================
# PostgreSQL 服务器
# ========================================
[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = true  # 默认禁用，按需启用

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://localhost/mydb"

# ========================================
# Slack 服务器
# ========================================
[servers.slack]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-slack"]
disabled = true  # 默认禁用

[servers.slack.env]
SLACK_BOT_TOKEN = "xoxb-your-token"

# ========================================
# 自定义服务器
# ========================================
[servers.custom]
command = "/usr/local/bin/my-server"
args = ["--port", "3000"]
disabled = false
```

---

## 🎯 推荐配置

### 开发环境

```toml
# 开发常用配置
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", 
        "$HOME/Projects",
        "/tmp"]
disabled = false

[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_dev_token"
```

### 生产环境

```toml
# 生产环境配置
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/var/app/data"]
disabled = false

[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = false

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://prod-server/db"
```

---

## 📚 下一步

### 使用 MCP
```bash
# 在 REPL 中使用
alius
> /tools

# AI 会自动使用 MCP 工具
> 读取文件内容
> 创建 GitHub issue
```

### 查看完整文档
- 完整指南: `MCP_USER_GUIDE.md`
- 快速参考: `MCP_QUICK_REFERENCE.md`

---

## 💡 提示

1. **从简单开始**: 先配置 filesystem 服务器测试
2. **路径安全**: 只配置必要的目录路径
3. **环境变量**: 敏感信息使用环境变量
4. **逐步添加**: 一次添加一个服务器，验证后再添加下一个
5. **禁用不用的**: 暂时不用的服务器设置 `disabled = true`

---

**配置完成后，重启 Alius 或重新运行 `alius mcp list` 即可使用！** 🚀
