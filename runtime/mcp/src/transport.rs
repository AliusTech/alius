//! Transport layer for MCP communication.

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Transport abstraction for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a JSON-RPC message
    async fn send(&mut self, message: Value) -> anyhow::Result<()>;

    /// Receive a JSON-RPC message
    async fn receive(&mut self) -> anyhow::Result<Value>;

    /// Close the transport
    async fn close(&mut self) -> anyhow::Result<()>;
}

/// Stdio transport for local MCP servers
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioTransport {
    /// Create a new stdio transport by spawning a process
    pub async fn new(command: &str, args: &[String]) -> anyhow::Result<Self> {
        tracing::debug!("Spawning MCP server: {} {:?}", command, args);

        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(&message)?;
        tracing::trace!("MCP send: {}", json);

        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        Ok(())
    }

    async fn receive(&mut self) -> anyhow::Result<Value> {
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;

        if line.is_empty() {
            anyhow::bail!("MCP server closed connection");
        }

        tracing::trace!("MCP recv: {}", line.trim());
        let value = serde_json::from_str(&line)?;

        Ok(value)
    }

    async fn close(&mut self) -> anyhow::Result<()> {
        tracing::debug!("Closing MCP transport");
        self.child.kill().await?;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Best effort kill on drop
        let _ = self.child.start_kill();
    }
}
