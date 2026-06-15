# MCP 添加快速参考卡片

## 🚀 3 步添加 MCP 服务器

### 1. 创建配置文件
```bash
mkdir -p ~/.alius/mcp
touch ~/.alius/mcp/servers.toml
```

### 2. 添加服务器配置
```bash
vim ~/.alius/mcp/servers.toml
```

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
disabled = false
```

### 3. 验证
```bash
alius mcp list
```

---

## 📝 常用配置模板

### Filesystem
```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
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

## ✅ 验证步骤

```bash
# 1. 列出服务器
alius mcp list

# 2. 启动服务器
alius mcp start filesystem

# 3. 查看工具
alius mcp tools filesystem
```

---

## 🐛 常见问题

### 配置文件位置
```
~/.alius/mcp/servers.toml
```

### Node.js 未安装
```bash
brew install node
```

### 权限错误
只配置你有权限的目录

---

查看完整指南: `HOW_TO_ADD_MCP.md`
