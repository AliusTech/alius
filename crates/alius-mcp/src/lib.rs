//! Alius MCP — Model Context Protocol client.
//!
//! Manages MCP server processes and communicates via JSON-RPC 2.0 over stdio.
//! Supports tools/list and tools/call methods.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::io::{BufRead, BufReader, Read, Write};

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// MCP configuration file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

/// A running MCP server instance.
pub struct McpServer {
    pub name: String,
    pub config: McpServerConfig,
    pub child: Child,
    request_id: u64,
}

/// MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// JSON-RPC 2.0 request.
#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error.
#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Load MCP config from file.
pub fn load_config() -> Result<McpConfig> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // Try project config first, then global
    let paths = vec![
        PathBuf::from("alius").join("mcp.json"),
        PathBuf::from(home).join(".alius").join("mcp.json"),
    ];

    for path in &paths {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: McpConfig = serde_json::from_str(&content)?;
            return Ok(config);
        }
    }

    Ok(McpConfig { servers: HashMap::new() })
}

/// List configured MCP servers (without starting them).
pub fn list_configured_servers() -> Result<Vec<(String, McpServerConfig)>> {
    let config = load_config()?;
    Ok(config.servers.into_iter().collect())
}

impl McpServer {
    /// Start an MCP server process.
    pub fn start(name: &str, config: &McpServerConfig) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

        let child = cmd.spawn()?;

        let mut server = McpServer {
            name: name.to_string(),
            config: config.clone(),
            child,
            request_id: 0,
        };

        // Initialize the MCP session
        server.initialize()?;

        Ok(server)
    }

    /// Send initialize request to the MCP server.
    fn initialize(&mut self) -> Result<()> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "alius",
                "version": "0.3.0"
            }
        });
        let _: serde_json::Value = self.request("initialize", Some(params))?;
        // Send initialized notification
        self.notify("notifications/initialized", None)?;
        Ok(())
    }

    /// List tools offered by this server.
    pub fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result: serde_json::Value = self.request("tools/list", None)?;
        let tools = result.get("tools")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter().filter_map(|v| {
                    serde_json::from_value(v.clone()).ok()
                }).collect()
            })
            .unwrap_or_default();
        Ok(tools)
    }

    /// Call a tool on this server.
    pub fn call_tool(&mut self, name: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });
        self.request("tools/call", Some(params))
    }

    /// Send a JSON-RPC request and wait for response.
    fn request<T: serde::de::DeserializeOwned>(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<T> {
        self.request_id += 1;
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let stdin = self.child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("No stdin"))?;
        let req_str = serde_json::to_string(&req)?;
        writeln!(stdin, "Content-Length: {}\r\n\r\n{}", req_str.len(), req_str)?;
        stdin.flush()?;

        // Read response
        let stdout = self.child.stdout.as_mut()
            .ok_or_else(|| anyhow::anyhow!("No stdout"))?;
        let mut reader = BufReader::new(stdout);

        // Read Content-Length header
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(len) = line.strip_prefix("Content-Length: ") {
                content_length = len.parse().unwrap_or(0);
            }
        }

        if content_length == 0 {
            anyhow::bail!("No Content-Length in MCP response");
        }

        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf)?;
        let resp: JsonRpcResponse = serde_json::from_slice(&buf)?;

        if let Some(err) = resp.error {
            anyhow::bail!("MCP error {}: {}", err.code, err.message);
        }

        resp.result
            .ok_or_else(|| anyhow::anyhow!("No result in MCP response"))
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.into()))
    }

    /// Send a JSON-RPC notification (no response expected).
    fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("No stdin"))?;
        let req_str = serde_json::to_string(&req)?;
        writeln!(stdin, "Content-Length: {}\r\n\r\n{}", req_str.len(), req_str)?;
        stdin.flush()?;
        Ok(())
    }

    /// Stop the MCP server process.
    pub fn stop(&mut self) -> Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}
