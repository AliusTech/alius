# MCP 快速参考卡片

## 🚀 5 分钟快速开始

### 1. 创建配置
```bash
mkdir -p ~/.alius/mcp
cat > ~/.alius/mcp/servers.toml << 'EOF'
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
disabled = false
EOF
```

### 2. 使用 CLI
```bash
alius mcp list        # 列出服务器
alius mcp tools       # 查看工具
```

### 3. 在 REPL 中使用
```bash
alius
> /tools              # 查看所有工具
> 读取文件内容        # AI 自动调用 MCP 工具
```

---

## 📋 常用命令

| 命令 | 说明 |
|------|------|
| `alius mcp list` | 列出配置的服务器 |
| `alius mcp start <server>` | 启动服务器 |
| `alius mcp tools <server>` | 列出工具 |
| `/tools` (REPL) | 查看所有工具 |

---

## ⚙️ 配置示例

### Filesystem
```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path"]
disabled = false
```

### GitHub
```toml
[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.github.env]
GITHUB_TOKEN = "ghp_your_token"
```

### PostgreSQL
```toml
[servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
disabled = false

[servers.postgres.env]
POSTGRES_CONNECTION_STRING = "postgresql://localhost/db"
```

---

## 🐛 故障排除

### 配置文件位置
```bash
~/.alius/mcp/servers.toml
```

### 检查配置
```bash
cat ~/.alius/mcp/servers.toml
alius mcp list
```

### 测试服务器
```bash
alius mcp start filesystem
alius mcp tools filesystem
```

---

## 📚 完整文档

查看完整指南: `MCP_USER_GUIDE.md`

---

快速、简单、强大 - 立即开始使用 MCP！
