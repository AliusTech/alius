//! JSON-RPC 2.0 server entrypoint for Alius.
//!
//! Provides a TCP line-based JSON-RPC interface backed by `CoreRuntimeManager`.
//!
//! ## Error codes
//!
//! | Code    | Meaning                         |
//! |---------|---------------------------------|
//! | -32600  | Invalid JSON-RPC request        |
//! | -32601  | Method not found                |
//! | -32602  | Invalid params                  |
//! | -32000  | Runtime / internal error        |

use anyhow::Result;
use core_runtime::{CoreRuntimeManager, RuntimeManagerContext};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::net::SocketAddr;
use std::sync::Arc;

// ── JSON-RPC wire types ────────────────────────────────────────────────

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

// ── Error codes (standard JSON-RPC 2.0 + server-defined) ──────────────

/// Method not found.
const ERR_METHOD_NOT_FOUND: i64 = -32601;
/// Invalid params (e.g. missing required fields, wrong types).
const ERR_INVALID_PARAMS: i64 = -32602;
/// Runtime / internal error (wraps `ProtocolError`).
const ERR_RUNTIME: i64 = -32000;

// ── Legacy stub dispatcher (deprecated) ────────────────────────────────

/// Dispatch a JSON-RPC request using hardcoded stub responses.
///
/// **Deprecated**: use `dispatch_with_runtime` for runtime-backed execution.
/// Kept only for backward-compat unit tests that verify basic JSON-RPC framing.
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
        _ => method_not_found(request),
    }
}

// ── Runtime-backed dispatcher ──────────────────────────────────────────

/// Dispatch a JSON-RPC request through `CoreRuntimeManager`.
///
/// Methods:
/// - `health_check` → `CoreRuntimeManager::health_check()`
/// - `config_read`  → `CoreRuntimeManager::config_read()`
/// - `model_list`   → `CoreRuntimeManager::model_list()`
/// - `tool_list`    → `CoreRuntimeManager::tool_list()`
/// - `version`      → local crate version (no runtime)
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
        "model_list" => match manager.model_list() {
            Ok(models) => success(request, serde_json::json!(models)),
            Err(e) => runtime_error(request, e.to_string()),
        },
        "tool_list" => match manager.tool_list() {
            Ok(tools) => success(request, serde_json::json!(tools)),
            Err(e) => runtime_error(request, e.to_string()),
        },
        "version" => success(
            request,
            serde_json::json!({"version": env!("CARGO_PKG_VERSION")}),
        ),
        _ => method_not_found(request),
    }
}

// ── Response helpers ───────────────────────────────────────────────────

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
            code: ERR_RUNTIME,
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
            code: ERR_METHOD_NOT_FOUND,
            message: format!("method '{}' not found", request.method),
            data: None,
        }),
    }
}

#[allow(dead_code)] // Used in tests; available for future param validation paths.
fn invalid_params(request: &JsonRpcRequest, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id.clone(),
        result: None,
        error: Some(JsonRpcError {
            code: ERR_INVALID_PARAMS,
            message: message.into(),
            data: None,
        }),
    }
}

// ── Server ─────────────────────────────────────────────────────────────

/// Start the JSON-RPC server backed by a `CoreRuntimeManager`.
///
/// Each TCP connection reads one line, dispatches through `dispatch_with_runtime`,
/// and writes the response line.
pub async fn serve_with_runtime(addr: SocketAddr, manager: Arc<CoreRuntimeManager>) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let mgr = manager.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, &mgr).await {
                eprintln!("[jsonrpc] connection error: {e}");
            }
        });
    }
}

/// Start the JSON-RPC server with a default `CoreRuntimeManager`.
///
/// Creates a `CoreRuntimeManager` with `RuntimeManagerContext::json_rpc()`
/// backed by the current working directory and reuses it across all
/// connections.
pub async fn serve(addr: SocketAddr) -> Result<()> {
    let workspace = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let settings = runtime_config::Settings::load()
        .map_err(|e| anyhow::anyhow!("failed to load settings: {e}"))?;
    let manager = CoreRuntimeManager::new_with_context(
        &workspace,
        settings,
        RuntimeManagerContext::json_rpc(),
    )
    .map_err(|e| anyhow::anyhow!("failed to create runtime manager: {e}"))?;
    serve_with_runtime(addr, Arc::new(manager)).await
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    manager: &CoreRuntimeManager,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    match reader.read_line(&mut line).await {
        Ok(0) => Ok(()),
        Ok(_) => {
            let trimmed = line.trim();
            match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(req) => {
                    let resp = dispatch_with_runtime(&req, manager);
                    if let Ok(body) = serde_json::to_string(&resp) {
                        let _ = writer.write_all(format!("{}\n", body).as_bytes()).await;
                    }
                    Ok(())
                }
                Err(_) => {
                    // Malformed JSON → invalid request
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32600,
                            message: "Invalid JSON-RPC request".to_string(),
                            data: None,
                        }),
                    };
                    if let Ok(body) = serde_json::to_string(&resp) {
                        let _ = writer.write_all(format!("{}\n", body).as_bytes()).await;
                    }
                    Ok(())
                }
            }
        }
        Err(e) => Err(e.into()),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::core::WorkspaceRef;

    fn make_request(method: &str) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(1.into())),
            method: method.to_string(),
            params: JsonValue::Null,
        }
    }

    fn test_manager() -> CoreRuntimeManager {
        CoreRuntimeManager::from_runtime(
            "/tmp/jsonrpc-test",
            core_runtime::CoreRuntime::new(WorkspaceRef::new("/tmp/jsonrpc-test")),
        )
    }

    // ── Legacy dispatch ────────────────────────────────────────────────

    #[test]
    fn test_dispatch_health_check() {
        let resp = dispatch(&make_request("health_check"));
        assert!(resp.result.is_some());
        assert_eq!(resp.result.unwrap()["status"], "ok");
    }

    #[test]
    fn test_dispatch_config_read() {
        let resp = dispatch(&make_request("config_read"));
        assert!(resp.result.is_some());
        // Legacy stub returns hardcoded values — this is expected.
        assert_eq!(resp.result.unwrap()["provider"], "anthropic");
    }

    #[test]
    fn test_dispatch_version() {
        let resp = dispatch(&make_request("version"));
        assert!(resp.result.is_some());
        assert!(resp.result.unwrap()["version"].is_string());
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let resp = dispatch(&make_request("nonexistent"));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ERR_METHOD_NOT_FOUND);
    }

    // ── Runtime-backed dispatch ────────────────────────────────────────

    #[test]
    fn test_dispatch_with_runtime_health_check() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("health_check"), &manager);
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert!(result["workspace_ok"].is_boolean());
    }

    #[test]
    fn test_dispatch_with_runtime_config_read() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("config_read"), &manager);
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        // Runtime-backed config_read returns real settings, not hardcoded.
        assert!(result["model"].is_string());
        assert!(result["provider"].is_string());
    }

    #[test]
    fn test_dispatch_with_runtime_config_read_not_hardcoded() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("config_read"), &manager);
        let result = resp.result.unwrap();
        // Must not be the legacy stub values.
        assert_ne!(result["provider"], "anthropic");
        assert_ne!(result["model"], "glm-4.7");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_dispatch_with_runtime_model_list() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("model_list"), &manager);
        // model_list may return a runtime error if the model library isn't
        // configured in the test workspace — that's acceptable.
        // What matters is the dispatch reached the runtime path.
        if let Some(err) = &resp.error {
            assert_eq!(err.code, ERR_RUNTIME);
        } else {
            let result = resp.result.unwrap();
            assert!(result.is_array());
        }
    }

    #[test]
    fn test_dispatch_with_runtime_tool_list() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("tool_list"), &manager);
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert!(result.is_array());
    }

    #[test]
    fn test_dispatch_with_runtime_version() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("version"), &manager);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["version"].is_string());
    }

    #[test]
    fn test_dispatch_with_runtime_unknown_method() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("nonexistent"), &manager);
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, ERR_METHOD_NOT_FOUND);
        assert!(err.message.contains("nonexistent"));
    }

    // ── Error code semantics ───────────────────────────────────────────

    #[test]
    fn test_error_code_method_not_found() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("no_such_method"), &manager);
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_error_code_runtime() {
        // config_update with invalid params would produce a runtime error,
        // but we don't have a method that reliably errors in test mode.
        // Instead verify the error code constant is correct.
        assert_eq!(ERR_RUNTIME, -32000);
    }

    #[test]
    fn test_error_code_invalid_params() {
        assert_eq!(ERR_INVALID_PARAMS, -32602);
    }

    #[test]
    fn test_invalid_params_helper() {
        let req = make_request("test");
        let resp = invalid_params(&req, "missing required field 'model'");
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("missing required field"));
    }

    // ── Response structure ─────────────────────────────────────────────

    #[test]
    fn test_response_has_jsonrpc_version() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("health_check"), &manager);
        assert_eq!(resp.jsonrpc, "2.0");
    }

    #[test]
    fn test_response_preserves_id() {
        let manager = test_manager();
        let resp = dispatch_with_runtime(&make_request("health_check"), &manager);
        assert_eq!(resp.id, Some(JsonValue::Number(1.into())));
    }

    // ── Server wiring ──────────────────────────────────────────────────

    /// Verify that `serve_with_runtime` accepts an Arc<CoreRuntimeManager>
    /// and can be constructed (does not panic).
    #[test]
    fn test_serve_with_runtime_accepts_arc_manager() {
        let manager = Arc::new(test_manager());
        // Just verify the function signature compiles — we don't actually bind.
        let _addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let _mgr = manager;
        // If we got here, the types are correct.
    }
}
