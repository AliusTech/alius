# MCP (Model Context Protocol) 使用指南

**版本**: v0.1.0-sprint1  
**更新日期**: 2026-06-17

---

## 📋 目录

1. [MCP 简介](#mcp-简介)
2. [快速开始](#快速开始)
3. [配置 MCP 服务器](#配置-mcp-服务器)
4. [使用 MCP CLI 命令](#使用-mcp-cli-命令)
5. [在 REPL 中使用](#在-repl-中使用)
6. [常见 MCP 服务器](#常见-mcp-服务器)
7. [故障排除](#故障排除)
8. [高级用法](#高级用法)

---

## 🎯 MCP 简介

### 什么是 MCP？

MCP (Model Context Protocol) 是一个开放协议，允许 AI 应用程序与外部工具和数据源集成。

### 为什么使用 MCP？

- **扩展能力**: 为 AI 添加文件系统、GitHub、数据库等能力
- **标准化**: 统一的工具接口，一次配置，到处使用
- **生态丰富**: 社区提供大量现成的 MCP 服务器

### Alius 中的 MCP

Alius 完整支持 MCP v2024-11-05 协议：
- ✅ 后台异步初始化
- ✅ 动态工具注册
- ✅ CLI 命令管理
- ✅ REPL 集成

---

## 🚀 快速开始

### 前置要求

- Alius CLI 已安装
- Node.js 16+ (用于运行 MCP 服务器)

### 3 分钟快速上手

#### 1. 创建配置文件

```bash
# 创建 MCP 配置目录
mkdir -p ~/.alius/mcp

# 创建配置文件
cat > ~/.alius/mcp/servers.toml << 'EOF'
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
disabled = false

[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false
EOF
```

#### 2. 测试 MCP

```bash
# 列出配置的服务器
alius mcp list

# 输出:
# Available MCP servers:
#   filesystem (enabled)
#   github (enabled)
```

#### 3. 查看工具

```bash
# 列出所有工具
alius mcp tools filesystem

# 或在 REPL 中
alius
> /tools
```

---

## ⚙️ 配置 MCP 服务器

### 配置文件位置

```bash
~/.alius/mcp/servers.toml
```

### 配置格式

```toml
[servers.<服务器名称>]
command = "<命令>"
args = ["<参数1>", "<参数2>", ...]
disabled = false  # true 表示禁用

[servers.<服务器名称>.env]
API_KEY = "your-api-key"  # 环境变量
```

### 完整配置示例

```toml
# 文件系统服务器
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp", "/Users/username/Documents"]
disabled = false

# GitHub 服务器
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_token_here"

# PostgreSQL 服务器
[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = false

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://user:pass@localhost/db"

# 自定义本地服务器
[servers.custom]
command = "/usr/local/bin/my-mcp-server"
args = ["--config", "/path/to/config.json"]
disabled = false
```

### 配置提示

#### 1. 路径配置
```toml
# ✅ 推荐：使用绝对路径
args = ["-y", "@modelcontextprotocol/server-filesystem", "/Users/username/Projects"]

# ❌ 避免：相对路径可能不工作
args = ["-y", "@modelcontextprotocol/server-filesystem", "./projects"]
```

#### 2. 环境变量
```toml
# 敏感信息使用环境变量
[servers.api.env]
API_KEY = "your-secret-key"
API_URL = "https://api.example.com"
```

#### 3. 禁用服务器
```toml
# 临时禁用服务器
[servers.slow-server]
command = "npx"
args = ["-y", "@example/slow-server"]
disabled = true  # 不会启动
```

---

## 🔧 使用 MCP CLI 命令

### 列出服务器

```bash
# 列出所有配置的服务器
alius mcp list

# 输出示例:
# Available MCP servers:
#   filesystem (enabled)
#   github (enabled)
#   postgres (disabled)
```

### 启动服务器

```bash
# 启动单个服务器
alius mcp start filesystem

# 输出:
# Starting MCP server: filesystem
# Server started successfully
# Available tools: 5
```

### 列出工具

```bash
# 列出服务器的所有工具
alius mcp tools filesystem

# 输出示例:
# Available tools from filesystem:
#   read_file - Read file content
#   write_file - Write content to file
#   list_directory - List directory contents
#   create_directory - Create a new directory
#   delete_file - Delete a file
```

### 停止服务器

```bash
# 停止服务器 (当前版本自动管理)
# 服务器在 Alius 退出时自动停止
```

---

## 💬 在 REPL 中使用

### 启动 REPL

```bash
alius
```

### 查看所有工具

```
> /tools

Built-in Tools:
  🔧 read_file - Read file content
  🔧 write_file - Write file to disk
  🔧 bash - Execute shell commands

📦 MCP Tools (2 servers, 8 tools):

  📦 filesystem (5 tools)
    🔧 read_file - Read file from filesystem
    🔧 write_file - Write file to filesystem
    🔧 list_directory - List directory contents
    🔧 create_directory - Create new directory
    🔧 delete_file - Delete a file
    
  📦 github (3 tools)
    🔧 create_issue - Create a GitHub issue
    🔧 list_repos - List GitHub repositories
    🔧 get_pr - Get pull request details
```

### 使用 MCP 工具

MCP 工具会自动注册到 Alius，AI 可以直接调用：

```
> 读取 /tmp/test.txt 文件的内容

AI 会自动选择并调用 filesystem.read_file 工具
```

```
> 在 GitHub 上创建一个 issue

AI 会自动选择并调用 github.create_issue 工具
```

---

## 🌐 常见 MCP 服务器

### 官方服务器

#### 1. Filesystem Server
**功能**: 文件系统操作

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allow"]
disabled = false
```

**工具**:
- `read_file` - 读取文件
- `write_file` - 写入文件
- `list_directory` - 列出目录
- `create_directory` - 创建目录
- `delete_file` - 删除文件

#### 2. GitHub Server
**功能**: GitHub 操作

```toml
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_token"
```

**工具**:
- `create_issue` - 创建 Issue
- `list_repos` - 列出仓库
- `get_pr` - 获取 PR 详情
- `create_pr` - 创建 PR

#### 3. PostgreSQL Server
**功能**: 数据库查询

```toml
[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = false

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://localhost/mydb"
```

**工具**:
- `query` - 执行 SQL 查询
- `list_tables` - 列出表
- `describe_table` - 查看表结构

#### 4. Slack Server
**功能**: Slack 集成

```toml
[servers.slack]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-slack"]
disabled = false

[servers.slack.env]
SLACK_BOT_TOKEN = "xoxb-your-token"
```

**工具**:
- `post_message` - 发送消息
- `list_channels` - 列出频道
- `get_users` - 获取用户列表

### 社区服务器

更多服务器请查看：
- https://github.com/modelcontextprotocol/servers
- https://mcp.run (MCP 服务器目录)

---

## 🐛 故障排除

### 问题 1: 没有找到 MCP 配置

**症状**:
```
No MCP servers configured.
```

**解决方案**:
```bash
# 检查配置文件是否存在
ls -la ~/.alius/mcp/servers.toml

# 如果不存在，创建配置
mkdir -p ~/.alius/mcp
# 然后添加配置（见上文）
```

### 问题 2: 服务器启动失败

**症状**:
```
Failed to start MCP server: filesystem
```

**解决方案**:
```bash
# 1. 检查 Node.js 是否安装
node --version

# 2. 手动测试服务器
npx -y @modelcontextprotocol/server-filesystem /tmp

# 3. 检查路径权限
ls -ld /tmp

# 4. 查看详细错误日志
# (当前版本日志在终端输出)
```

### 问题 3: 工具列表为空

**症状**:
```
alius mcp tools filesystem
# 没有输出
```

**解决方案**:
```bash
# 1. 确认服务器已启动
alius mcp list

# 2. 检查服务器是否被禁用
cat ~/.alius/mcp/servers.toml | grep disabled

# 3. 重启 Alius
```

### 问题 4: GitHub Token 问题

**症状**:
```
GitHub authentication failed
```

**解决方案**:
```bash
# 1. 创建 GitHub Personal Access Token
# 访问: https://github.com/settings/tokens

# 2. 添加到配置
[servers.github.env]
GITHUB_TOKEN = "ghp_your_new_token"

# 3. 确保 Token 有正确的权限
# - repo (完整仓库访问)
# - read:org (读取组织)
```

### 问题 5: 权限错误

**症状**:
```
Permission denied: /path/to/file
```

**解决方案**:
```bash
# 1. 检查文件权限
ls -l /path/to/file

# 2. 确保配置的路径可访问
# filesystem 服务器只能访问配置的目录

# 3. 更新配置允许更多路径
[servers.filesystem]
args = ["-y", "@modelcontextprotocol/server-filesystem", 
        "/tmp", 
        "/Users/username/Documents",
        "/Users/username/Projects"]
```

---

## 🔬 高级用法

### 1. 创建自定义 MCP 服务器

#### 简单示例 (Node.js)

```javascript
// my-server.js
const { Server } = require('@modelcontextprotocol/sdk/server');
const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio');

const server = new Server({
  name: 'my-custom-server',
  version: '1.0.0',
}, {
  capabilities: {
    tools: {},
  },
});

// 注册工具
server.setRequestHandler('tools/list', async () => {
  return {
    tools: [
      {
        name: 'greet',
        description: 'Say hello',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string' },
          },
          required: ['name'],
        },
      },
    ],
  };
});

server.setRequestHandler('tools/call', async (request) => {
  const { name, arguments: args } = request.params;
  
  if (name === 'greet') {
    return {
      content: [{
        type: 'text',
        text: `Hello, ${args.name}!`,
      }],
    };
  }
});

// 启动
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main();
```

#### 配置自定义服务器

```toml
[servers.custom]
command = "node"
args = ["/path/to/my-server.js"]
disabled = false
```

### 2. 使用 Docker 运行 MCP 服务器

```bash
# Dockerfile
FROM node:18
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY server.js ./
CMD ["node", "server.js"]
```

```toml
# servers.toml
[servers.docker-server]
command = "docker"
args = ["run", "-i", "my-mcp-server"]
disabled = false
```

### 3. 多环境配置

```bash
# 开发环境
~/.alius/mcp/servers.dev.toml

# 生产环境
~/.alius/mcp/servers.prod.toml

# 使用环境变量切换
export ALIUS_MCP_CONFIG=~/.alius/mcp/servers.prod.toml
alius
```

### 4. 调试 MCP 服务器

```bash
# 启用详细日志
export RUST_LOG=debug
alius

# 或在配置中添加调试参数
[servers.debug-server]
command = "node"
args = ["server.js", "--debug", "--verbose"]
```

### 5. 性能优化

#### 延迟加载
```toml
# 只在需要时启动服务器
[servers.heavy-server]
command = "npx"
args = ["-y", "@example/heavy-server"]
disabled = true  # 手动启动: alius mcp start heavy-server
```

#### 连接池
```toml
# 数据库服务器使用连接池
[servers.postgres.env]
POSTGRES_POOL_SIZE = "10"
POSTGRES_POOL_TIMEOUT = "30000"
```

---

## 📚 最佳实践

### 1. 安全性

```toml
# ✅ 使用环境变量存储敏感信息
[servers.api.env]
API_KEY = "secret-key"

# ❌ 不要硬编码敏感信息
# args = ["--api-key", "secret-key"]  # 不推荐
```

### 2. 路径限制

```toml
# ✅ 只允许必要的路径
[servers.filesystem]
args = ["-y", "@modelcontextprotocol/server-filesystem", 
        "/Users/username/Projects"]  # 仅项目目录

# ❌ 避免过于宽泛的权限
# args = ["-y", "@modelcontextprotocol/server-filesystem", "/"]  # 危险！
```

### 3. 资源管理

```toml
# 对于资源密集型服务器，默认禁用
[servers.ml-model]
command = "python"
args = ["/path/to/ml-server.py"]
disabled = true  # 按需启动
```

### 4. 错误处理

```bash
# 测试配置后再使用
alius mcp list  # 检查配置是否正确
alius mcp start server-name  # 测试启动
alius mcp tools server-name  # 验证工具
```

---

## 📖 相关资源

### 官方文档
- MCP 官方文档: https://modelcontextprotocol.io
- MCP 服务器列表: https://github.com/modelcontextprotocol/servers
- MCP 规范: https://spec.modelcontextprotocol.io

### Alius 文档
- MCP 架构设计: `.alius/workspace/MCP_RUNTIME_DESIGN.md`
- 命令参考: `.alius/workspace/COMMANDS.md`
- Sprint 1 报告: `.alius/workspace/SPRINT_1_FINAL_REPORT.md`

### 社区
- GitHub Issues: [项目 Issues]
- 讨论区: [Discussion Forum]

---

## 🆘 获取帮助

### 查看帮助
```bash
# MCP 命令帮助
alius mcp --help

# 通用帮助
alius --help
```

### 联系支持
- 查看文档: `.alius/workspace/`
- 提交 Issue: [GitHub]
- 社区讨论: [Forum]

---

**文档版本**: v0.1.0-sprint1  
**最后更新**: 2026-06-17  
**维护者**: Kiro (Claude)

---

**开始使用 MCP，扩展 Alius 的能力！** 🚀
