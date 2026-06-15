# 06 - MCP 协议集成

## 📋 概述

Model Context Protocol (MCP) 是 Anthropic 提出的标准化协议，用于 AI 应用与外部工具、数据源的交互。集成 MCP 可以让 Alius 接入庞大的 MCP 工具生态系统。

## 🎯 目标

1. 实现完整的 MCP 客户端协议
2. 支持本地和远程 MCP 服务器
3. 动态发现和加载 MCP 工具
4. 提供 MCP 服务器管理界面

## 📊 现状分析

### Alius 当前状态
- ✅ 有基础的工具注册和调用机制
- ✅ runtime-tools 模块提供工具抽象
- ❌ 缺少 MCP 协议支持
- ❌ 工具发现机制是静态的
- ❌ 没有远程工具支持

### 竞品实现

#### claude-code
```typescript
// @modelcontextprotocol/sdk 集成
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

// MCP 服务器配置
interface McpServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

// MCP 客户端管理器
class McpManager {
  private clients: Map<string, Client> = new Map();
  
  async connectServer(name: string, config: McpServerConfig) {
    const transport = new StdioClientTransport({
      command: config.command,
      args: config.args,
      env: config.env,
    });
    
    const client = new Client({ name, version: '1.0.0' }, {
      capabilities: {
        tools: {},
        resources: {},
        prompts: {},
      }
    });
    
    await client.connect(transport);
    this.clients.set(name, client);
    
    // 工具发现
    const { tools } = await client.listTools();
    return tools;
  }
  
  async callTool(server: string, toolName: string, args: any) {
    const client = this.clients.get(server);
    const result = await client.callTool({ name: toolName, arguments: args });
    return result;
  }
}
```

#### codex
```rust
// MCP 客户端实现 (伪代码，基于 codex-rs 分析)
pub struct McpClient {
    transport: Box<dyn Transport>,
    capabilities: ClientCapabilities,
}

impl McpClient {
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": self.next_id(),
        });
        
        let response = self.transport.send(request).await?;
        Ok(serde_json::from_value(response["result"]["tools"])?)
    }
    
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolResult> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
            },
            "id": self.next_id(),
        });
        
        let response = self.transport.send(request).await?;
        Ok(serde_json::from_value(response["result"])?)
    }
}
```

## 💡 改进方案

### 1. MCP 协议层

**新增模块**: `runtime/mcp/`

```rust
// runtime/mcp/src/lib.rs
pub mod client;
pub mod transport;
pub mod protocol;
pub mod registry;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP 协议版本
pub const MCP_VERSION: &str = "2024-11-05";

/// MCP 客户端能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
    pub prompts: Option<PromptsCapability>,
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// MCP 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

/// MCP 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String, mime_type: Option<String> },
}
```

### 2. 传输层实现

```rust
// runtime/mcp/src/transport.rs
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, message: Value) -> anyhow::Result<()>;
    async fn receive(&mut self) -> anyhow::Result<Value>;
    async fn close(&mut self) -> anyhow::Result<()>;
}

/// Stdio 传输层（本地进程）
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioTransport {
    pub async fn new(command: &str, args: &[String]) -> anyhow::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        
        Ok(Self { child, stdin, stdout })
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(&message)?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }
    
    async fn receive(&mut self) -> anyhow::Result<Value> {
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        Ok(serde_json::from_str(&line)?)
    }
    
    async fn close(&mut self) -> anyhow::Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

/// SSE 传输层（远程 HTTP 服务）
pub struct SseTransport {
    url: String,
    client: reqwest::Client,
}

#[async_trait]
impl Transport for SseTransport {
    async fn send(&mut self, message: Value) -> anyhow::Result<()> {
        self.client
            .post(&self.url)
            .json(&message)
            .send()
            .await?;
        Ok(())
    }
    
    async fn receive(&mut self) -> anyhow::Result<Value> {
        // SSE 事件流接收
        todo!("Implement SSE event stream")
    }
    
    async fn close(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
```

### 3. MCP 客户端

```rust
// runtime/mcp/src/client.rs
use crate::protocol::*;
use crate::transport::Transport;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct McpClient {
    name: String,
    version: String,
    transport: Arc<Mutex<Box<dyn Transport>>>,
    request_id: AtomicU64,
    server_info: Option<ServerInfo>,
    server_capabilities: Option<ServerCapabilities>,
}

impl McpClient {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        transport: Box<dyn Transport>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            transport: Arc::new(Mutex::new(transport)),
            request_id: AtomicU64::new(1),
            server_info: None,
            server_capabilities: None,
        }
    }
    
    /// 初始化连接
    pub async fn initialize(&mut self, capabilities: ClientCapabilities) -> anyhow::Result<()> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_VERSION,
                "capabilities": capabilities,
                "clientInfo": {
                    "name": self.name,
                    "version": self.version,
                }
            },
            "id": id,
        });
        
        let mut transport = self.transport.lock().await;
        transport.send(request).await?;
        
        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Initialize failed: {}", error);
        }
        
        let result = response["result"].clone();
        self.server_info = serde_json::from_value(result["serverInfo"].clone()).ok();
        self.server_capabilities = serde_json::from_value(result["capabilities"].clone()).ok();
        
        // 发送 initialized 通知
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        transport.send(notification).await?;
        
        Ok(())
    }
    
    /// 列出可用工具
    pub async fn list_tools(&self) -> anyhow::Result<Vec<McpTool>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": id,
        });
        
        let mut transport = self.transport.lock().await;
        transport.send(request).await?;
        
        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("List tools failed: {}", error);
        }
        
        let tools: Vec<McpTool> = serde_json::from_value(response["result"]["tools"].clone())?;
        Ok(tools)
    }
    
    /// 调用工具
    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": name.into(),
                "arguments": arguments,
            },
            "id": id,
        });
        
        let mut transport = self.transport.lock().await;
        transport.send(request).await?;
        
        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Tool call failed: {}", error);
        }
        
        let result: McpToolResult = serde_json::from_value(response["result"].clone())?;
        Ok(result)
    }
    
    /// 列出资源
    pub async fn list_resources(&self) -> anyhow::Result<Vec<Resource>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "resources/list",
            "id": id,
        });
        
        let mut transport = self.transport.lock().await;
        transport.send(request).await?;
        
        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("List resources failed: {}", error);
        }
        
        let resources: Vec<Resource> = serde_json::from_value(response["result"]["resources"].clone())?;
        Ok(resources)
    }
    
    /// 读取资源
    pub async fn read_resource(&self, uri: impl Into<String>) -> anyhow::Result<Vec<Content>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "resources/read",
            "params": {
                "uri": uri.into(),
            },
            "id": id,
        });
        
        let mut transport = self.transport.lock().await;
        transport.send(request).await?;
        
        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Read resource failed: {}", error);
        }
        
        let contents: Vec<Content> = serde_json::from_value(response["result"]["contents"].clone())?;
        Ok(contents)
    }
    
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}
```

### 4. MCP 注册表

```rust
// runtime/mcp/src/registry.rs
use crate::client::McpClient;
use crate::protocol::McpTool;
use crate::transport::{StdioTransport, Transport};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

pub struct McpRegistry {
    servers: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
    configs: HashMap<String, McpServerConfig>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            configs: HashMap::new(),
        }
    }
    
    /// 从配置文件加载 MCP 服务器
    pub fn load_config(&mut self, config_path: &std::path::Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(config_path)?;
        self.configs = serde_json::from_str(&content)?;
        Ok(())
    }
    
    /// 连接指定的 MCP 服务器
    pub async fn connect_server(&self, name: &str) -> anyhow::Result<()> {
        let config = self.configs.get(name)
            .ok_or_else(|| anyhow::anyhow!("Server not found: {}", name))?;
        
        if config.disabled {
            anyhow::bail!("Server is disabled: {}", name);
        }
        
        let transport = StdioTransport::new(&config.command, &config.args).await?;
        let mut client = McpClient::new("alius", env!("CARGO_PKG_VERSION"), Box::new(transport));
        
        client.initialize(crate::ClientCapabilities {
            tools: Some(crate::ToolsCapability { list_changed: Some(true) }),
            resources: Some(crate::ResourcesCapability {}),
            prompts: Some(crate::PromptsCapability {}),
            sampling: None,
        }).await?;
        
        self.servers.write().await.insert(name.to_string(), Arc::new(client));
        Ok(())
    }
    
    /// 连接所有配置的 MCP 服务器
    pub async fn connect_all(&self) -> anyhow::Result<()> {
        for (name, config) in &self.configs {
            if config.disabled {
                continue;
            }
            
            match self.connect_server(name).await {
                Ok(_) => log::info!("Connected MCP server: {}", name),
                Err(e) => log::warn!("Failed to connect MCP server {}: {}", name, e),
            }
        }
        Ok(())
    }
    
    /// 获取所有 MCP 工具
    pub async fn list_all_tools(&self) -> anyhow::Result<HashMap<String, Vec<McpTool>>> {
        let servers = self.servers.read().await;
        let mut all_tools = HashMap::new();
        
        for (server_name, client) in servers.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    all_tools.insert(server_name.clone(), tools);
                }
                Err(e) => {
                    log::warn!("Failed to list tools from {}: {}", server_name, e);
                }
            }
        }
        
        Ok(all_tools)
    }
    
    /// 调用 MCP 工具
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<crate::McpToolResult> {
        let servers = self.servers.read().await;
        let client = servers.get(server)
            .ok_or_else(|| anyhow::anyhow!("Server not connected: {}", server))?;
        
        client.call_tool(tool, arguments).await
    }
}
```

### 5. 与 runtime-tools 集成

```rust
// runtime/tools/src/mcp_bridge.rs
use runtime_mcp::{McpRegistry, McpTool, McpToolResult};
use crate::{AliusTool, ToolContext, ToolResult};
use async_trait::async_trait;

/// MCP 工具桥接器
pub struct McpToolBridge {
    registry: Arc<McpRegistry>,
    server_name: String,
    tool_name: String,
    tool_def: McpTool,
}

#[async_trait]
impl AliusTool for McpToolBridge {
    fn name(&self) -> &str {
        &self.tool_name
    }
    
    fn description(&self) -> &str {
        self.tool_def.description.as_deref().unwrap_or("")
    }
    
    fn input_schema(&self) -> serde_json::Value {
        self.tool_def.input_schema.clone()
    }
    
    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let result = self.registry.call_tool(&self.server_name, &self.tool_name, args).await?;
        
        // 转换 MCP 结果为 Alius ToolResult
        let output = result.content.iter()
            .map(|content| match content {
                runtime_mcp::Content::Text { text } => text.clone(),
                runtime_mcp::Content::Image { .. } => "[Image]".to_string(),
                runtime_mcp::Content::Resource { uri, .. } => format!("[Resource: {}]", uri),
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(ToolResult {
            success: !result.is_error.unwrap_or(false),
            output,
        })
    }
}

/// MCP 工具注册
pub async fn register_mcp_tools(
    registry: &mut crate::ToolRegistry,
    mcp_registry: Arc<McpRegistry>,
) -> anyhow::Result<()> {
    let all_tools = mcp_registry.list_all_tools().await?;
    
    for (server_name, tools) in all_tools {
        for tool in tools {
            let bridge = McpToolBridge {
                registry: mcp_registry.clone(),
                server_name: server_name.clone(),
                tool_name: tool.name.clone(),
                tool_def: tool,
            };
            
            registry.register(Arc::new(bridge))?;
        }
    }
    
    Ok(())
}
```

### 6. 配置文件格式

```toml
# .alius/mcp/servers.toml
[servers]

[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed"]
disabled = false

[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
disabled = false

[servers.postgres]
command = "docker"
args = ["run", "-i", "mcp/postgres", "postgresql://localhost/mydb"]
disabled = true

[servers.custom-tool]
command = "/usr/local/bin/my-mcp-server"
args = []
disabled = false

[servers.custom-tool.env]
API_KEY = "${CUSTOM_API_KEY}"
BASE_URL = "https://api.example.com"
```

### 7. CLI 命令

```rust
// entrypoints/cli/src/cli.rs
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    // ... 其他命令
    
    /// MCP 服务器管理
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

#[derive(Debug, clap::Subcommand)]
pub enum McpCommand {
    /// 列出配置的 MCP 服务器
    List,
    
    /// 连接 MCP 服务器
    Connect {
        /// 服务器名称
        name: String,
    },
    
    /// 断开 MCP 服务器
    Disconnect {
        /// 服务器名称
        name: String,
    },
    
    /// 列出 MCP 工具
    Tools {
        /// 服务器名称（可选，默认列出所有）
        server: Option<String>,
    },
    
    /// 测试 MCP 工具
    Test {
        /// 服务器名称
        server: String,
        /// 工具名称
        tool: String,
        /// 工具参数（JSON）
        #[arg(long)]
        args: Option<String>,
    },
}
```

### 8. TUI 集成

```rust
// entrypoints/cli/src/tui/workspace/mod.rs
impl WorkspaceState {
    pub async fn cmd_mcp_list(&self) -> Result<String> {
        let registry = self.mcp_registry.as_ref()
            .ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;
        
        let all_tools = registry.list_all_tools().await?;
        
        let mut output = String::new();
        output.push_str("MCP Servers and Tools:\n\n");
        
        for (server, tools) in all_tools {
            output.push_str(&format!("📦 {}\n", server));
            for tool in tools {
                output.push_str(&format!("  🔧 {} - {}\n", 
                    tool.name, 
                    tool.description.unwrap_or_default()
                ));
            }
            output.push('\n');
        }
        
        Ok(output)
    }
}
```

## 🚀 实施步骤

### 阶段 1：基础协议实现（2周）
1. ✅ 创建 `runtime/mcp` 模块
2. ✅ 实现 MCP 协议数据结构
3. ✅ 实现 Stdio 传输层
4. ✅ 实现基础 MCP 客户端
5. ✅ 单元测试

### 阶段 2：工具集成（1周）
1. ✅ 实现 MCP 注册表
2. ✅ 实现 MCP 工具桥接
3. ✅ 与 runtime-tools 集成
4. ✅ 配置文件加载
5. ✅ 集成测试

### 阶段 3：CLI 命令（1周）
1. ✅ 实现 `alius mcp` 命令族
2. ✅ 实现服务器管理命令
3. ✅ 实现工具列表和测试命令
4. ✅ 文档更新

### 阶段 4：高级特性（2周）
1. ✅ 实现 SSE 传输层（远程服务器）
2. ✅ 实现 Resources 支持
3. ✅ 实现 Prompts 支持
4. ✅ 实现工具变更通知
5. ✅ TUI 集成

### 阶段 5：生态集成（持续）
1. 🔄 集成常用 MCP 服务器
2. 🔄 创建官方 MCP 服务器推荐列表
3. 🔄 文档和教程
4. 🔄 社区工具贡献

## 📈 预期收益

1. **工具生态**: 接入整个 MCP 生态系统的工具
2. **扩展性**: 无需修改核心代码即可添加新工具
3. **标准化**: 遵循 Anthropic 标准，与其他 AI 工具兼容
4. **远程能力**: 支持远程 MCP 服务器，云端工具
5. **社区活力**: 社区可以贡献 MCP 服务器

## ⚠️ 风险和缓解

### 风险 1：MCP 服务器稳定性
**缓解**: 实现超时机制、自动重连、降级策略

### 风险 2：性能开销
**缓解**: 工具列表缓存、连接池、懒加载

### 风险 3：安全问题
**缓解**: 沙箱执行、权限控制、配置验证

### 风险 4：协议演进
**缓解**: 版本协商、向后兼容、优雅降级

## 📚 参考资源

- [MCP Specification](https://modelcontextprotocol.io/docs)
- [MCP SDK TypeScript](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP Servers](https://github.com/modelcontextprotocol/servers)
- [Claude Code MCP 实现](https://github.com/anthropics/claude-code)

---

最后更新：2026-06-15
