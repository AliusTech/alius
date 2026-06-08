//! JSON-RPC 2.0 server entrypoint for Alius.
//!
//! Provides a lightweight TCP line-based JSON-RPC interface to CoreRuntime.

use anyhow::Result;
use core_runtime::CoreRuntimeManager;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::net::SocketAddr;

/// JSON-RPC request.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<JsonValue>,
    pub method: String,
    #[serde(default)]
    pub params: JsonValue,
}

/// JSON-RPC response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}

/// Dispatch a JSON-RPC request.
pub fn dispatch(request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "health_check" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        },
        "config_read" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({
                "provider": "anthropic",
                "model": "glm-4.7",
            })),
            error: None,
        },
        "version" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({"version": env!("CARGO_PKG_VERSION")})),
            error: None,
        },
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("method '{}' not found", request.method),
                data: None,
            }),
        },
    }
}

/// Dispatch a JSON-RPC request through CoreRuntimeManager.
pub fn dispatch_with_runtime(
    request: &JsonRpcRequest,
    manager: &CoreRuntimeManager,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "health_check" => match manager.health_check() {
            Ok(report) => success(request, serde_json::json!(report)),
            Err(e) => runtime_error(request, e.to_string()),
        },
        "config_read" => match manager.config_read() {
            Ok(snapshot) => success(request, serde_json::json!(snapshot)),
            Err(e) => runtime_error(request, e.to_string()),
        },
        "version" => success(
            request,
            serde_json::json!({"version": env!("CARGO_PKG_VERSION")}),
        ),
        _ => method_not_found(request),
    }
}

fn success(request: &JsonRpcRequest, result: JsonValue) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id.clone(),
        result: Some(result),
        error: None,
    }
}

fn runtime_error(request: &JsonRpcRequest, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id.clone(),
        result: None,
        error: Some(JsonRpcError {
            code: -32000,
            message,
            data: None,
        }),
    }
}

fn method_not_found(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id.clone(),
        result: None,
        error: Some(JsonRpcError {
            code: -32601,
            message: format!("method '{}' not found", request.method),
            data: None,
        }),
    }
}

/// Start the JSON-RPC server.
pub async fn serve(addr: SocketAddr) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {}
                Ok(_) => {
                    let trimmed = line.trim();
                    if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
                        let resp = dispatch(&req);
                        if let Ok(body) = serde_json::to_string(&resp) {
                            let _ = writer.write_all(format!("{}\n", body).as_bytes()).await;
                        }
                    }
                }
                Err(_) => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::core::WorkspaceRef;

    #[test]
    fn test_dispatch_health_check() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(1.into())),
            method: "health_check".to_string(),
            params: JsonValue::Null,
        };
        let resp = dispatch(&req);
        assert!(resp.result.is_some());
        assert!(resp.result.unwrap()["status"] == "ok");
    }

    #[test]
    fn test_dispatch_config_read() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(2.into())),
            method: "config_read".to_string(),
            params: JsonValue::Null,
        };
        let resp = dispatch(&req);
        assert!(resp.result.is_some());
        assert!(resp.result.unwrap()["model"].is_string());
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(3.into())),
            method: "nonexistent".to_string(),
            params: JsonValue::Null,
        };
        let resp = dispatch(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_dispatch_with_runtime_health_check() {
        let manager = CoreRuntimeManager::from_runtime(
            "/tmp/jsonrpc-test",
            core_runtime::CoreRuntime::new(WorkspaceRef::new("/tmp/jsonrpc-test")),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(4.into())),
            method: "health_check".to_string(),
            params: JsonValue::Null,
        };
        let resp = dispatch_with_runtime(&req, &manager);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["workspace_ok"].is_boolean());
    }

    #[test]
    fn test_dispatch_with_runtime_config_read() {
        let manager = CoreRuntimeManager::from_runtime(
            "/tmp/jsonrpc-test",
            core_runtime::CoreRuntime::new(WorkspaceRef::new("/tmp/jsonrpc-test")),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(5.into())),
            method: "config_read".to_string(),
            params: JsonValue::Null,
        };
        let resp = dispatch_with_runtime(&req, &manager);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["model"].is_string());
    }
}
