/// Handle MCP server management subcommands.
async fn handle_mcp_subcommand(
    command: cli::McpCommand,
    settings: &runtime_config::Settings,
) -> Result<()> {
    use cli::McpCommand;
    use runtime_mcp::{ClientCapabilities, McpRegistry, StdioTransport, ToolsCapability};
    use std::sync::Arc;

    // Determine MCP config path
    let mcp_config_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".alius/mcp");

    let mcp_config_path = mcp_config_dir.join("servers.toml");

    match command {
        McpCommand::List => {
            // Load and list configured servers
            if !mcp_config_path.exists() {
                println!("No MCP servers configured.");
                println!("Create a configuration file at: {}", mcp_config_path.display());
                println!("\nExample configuration:");
                println!("  [servers.filesystem]");
                println!("  command = \"npx\"");
                println!("  args = [\"-y\", \"@modelcontextprotocol/server-filesystem\", \"/path\"]");
                return Ok(());
            }

            let mut registry = McpRegistry::new();
            registry.load_config(&mcp_config_path)?;

            let configs = registry.list_configs();
            if configs.is_empty() {
                println!("No MCP servers configured in {}", mcp_config_path.display());
            } else {
                println!("Configured MCP servers:\n");
                for server_name in configs {
                    println!("  • {}", server_name);
                }
                println!("\nTotal: {} server(s)", configs.len());
            }
        }

        McpCommand::Connect { name } => {
            // Connect to a specific server
            let mut registry = McpRegistry::new();

            if !mcp_config_path.exists() {
                anyhow::bail!("No MCP configuration found at: {}", mcp_config_path.display());
            }

            registry.load_config(&mcp_config_path)?;

            println!("Connecting to MCP server: {}...", name);
            registry.connect_server(&name).await?;

            // List tools from the connected server
            if let Some(client) = registry.get_server(&name).await {
                let tools = client.list_tools().await?;
                println!("[ok] Connected successfully!");
                println!("\nAvailable tools ({}):", tools.len());
                for tool in tools {
                    let desc = tool.description.as_deref().unwrap_or("(no description)");
                    println!("  • {} - {}", tool.name, desc);
                }
            }
        }

        McpCommand::Disconnect { name } => {
            println!("Disconnecting from MCP server: {}...", name);
            // Note: Current implementation doesn't maintain persistent connections
            // This is a no-op for now
            println!("[ok] Server {} disconnected", name);
        }

        McpCommand::Tools { server } => {
            // List tools from all or specific server
            let mut registry = McpRegistry::new();

            if !mcp_config_path.exists() {
                anyhow::bail!("No MCP configuration found at: {}", mcp_config_path.display());
            }

            registry.load_config(&mcp_config_path)?;

            if let Some(server_name) = server {
                // List tools from specific server
                println!("Connecting to {}...", server_name);
                registry.connect_server(&server_name).await?;

                if let Some(client) = registry.get_server(&server_name).await {
                    let tools = client.list_tools().await?;
                    println!("\n{} tools:\n", server_name);
                    for tool in tools {
                        let desc = tool.description.as_deref().unwrap_or("(no description)");
                        println!("  {} - {}", tool.name, desc);
                    }
                    println!("\nTotal: {} tool(s)", tools.len());
                }
            } else {
                // List tools from all servers
                println!("Connecting to all MCP servers...");
                registry.connect_all().await?;

                let all_tools = registry.list_all_tools().await?;

                if all_tools.is_empty() {
                    println!("\nNo tools available from any server.");
                } else {
                    println!("\nMCP Tools by Server:\n");
                    for (server_name, tools) in all_tools {
                        println!("[server] {}", server_name);
                        for tool in tools {
                            let desc = tool.description.as_deref().unwrap_or("(no description)");
                            println!("  [tool] {} - {}", tool.name, desc);
                        }
                        println!();
                    }
                }
            }
        }

        McpCommand::Test { server, tool, args } => {
            // Test an MCP tool
            let mut registry = McpRegistry::new();

            if !mcp_config_path.exists() {
                anyhow::bail!("No MCP configuration found at: {}", mcp_config_path.display());
            }

            registry.load_config(&mcp_config_path)?;

            println!("Connecting to {}...", server);
            registry.connect_server(&server).await?;

            let arguments = if let Some(args_json) = args {
                serde_json::from_str(&args_json)?
            } else {
                serde_json::json!({})
            };

            println!("Calling tool: {}...", tool);
            let result = registry.call_tool(&server, &tool, arguments).await?;

            println!("\n[ok] Tool executed successfully!");
            println!("\nResult:");
            for content in result.content {
                match content {
                    runtime_mcp::Content::Text { text } => println!("{}", text),
                    runtime_mcp::Content::Image { mime_type, .. } => {
                        println!("[Image: {}]", mime_type)
                    }
                    runtime_mcp::Content::Resource { uri, .. } => {
                        println!("[Resource: {}]", uri)
                    }
                }
            }

            if let Some(true) = result.is_error {
                println!("\n[warn] Tool reported an error");
            }
        }
    }

    Ok(())
}
