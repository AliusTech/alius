//! HTTP request tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel, ConfirmationRequest};
use alius_protocol::AliusError;

pub struct HttpRequestTool;

#[async_trait]
impl AliusTool for HttpRequestTool {
    fn name(&self) -> &'static str {
        "http_request"
    }

    fn description(&self) -> &'static str {
        "Make an HTTP request. Supports GET, POST, PUT, DELETE methods."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to request"
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE"],
                    "default": "GET"
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP headers as key-value pairs"
                },
                "body": {
                    "type": "string",
                    "description": "Request body (for POST/PUT)"
                }
            },
            "required": ["url"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn requires_confirmation(&self, args: &JsonValue) -> bool {
        // Require confirmation for POST, PUT, DELETE
        let method = args["method"].as_str().unwrap_or("GET");
        method != "GET"
    }

    fn confirmation_request(&self, args: &JsonValue) -> Option<ConfirmationRequest> {
        if self.requires_confirmation(args) {
            Some(ConfirmationRequest {
                tool_name: self.name().to_string(),
                operation: format!("HTTP {}", args["method"].as_str().unwrap_or("GET")),
                details: format!("URL: {}", args["url"].as_str().unwrap_or("?")),
            })
        } else {
            None
        }
    }

    async fn execute(
        &self,
        args: JsonValue,
        _ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let url = args["url"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'url' argument".to_string()))?;

        let method = args["method"].as_str().unwrap_or("GET");
        let headers = args["headers"].as_object();
        let body = args["body"].as_str();

        let client = reqwest::Client::new();
        let mut request = match method {
            "GET" => client.get(url),
            "POST" => {
                let req = client.post(url);
                if let Some(b) = body {
                    req.body(b.to_string())
                } else {
                    req
                }
            }
            "PUT" => {
                let req = client.put(url);
                if let Some(b) = body {
                    req.body(b.to_string())
                } else {
                    req
                }
            }
            "DELETE" => client.delete(url),
            _ => return Err(AliusError::Agent(format!("Unsupported method: {}", method))),
        };

        // Add headers
        if let Some(h) = headers {
            for (key, value) in h {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| AliusError::Agent(format!("Request failed: {}", e)))?;

        let status = response.status().as_u16();
        let body = response.text()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to read response: {}", e)))?;

        // Truncate if too long
        let truncated = if body.len() > 5000 {
            format!("{}... (truncated)", &body[..5000])
        } else {
            body
        };

        Ok(ToolResult::success(format!("Status: {}\n{}", status, truncated)))
    }
}