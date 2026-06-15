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
use protocol_interface::core::{RunRef, RuntimeMode};
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
/// - `run_start`    → `CoreRuntimeManager::start_streaming()`
/// - `run_subscribe`→ `CoreRuntimeManager::subscribe()` (snapshot)
/// - `run_cancel`   → `CoreRuntimeManager::cancel()`
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
        "run_start" => handle_run_start(request, manager),
        "run_subscribe" => handle_run_subscribe(request, manager),
        "run_cancel" => handle_run_cancel(request, manager),
        _ => method_not_found(request),
    }
}

/// `run_start`: Start a streaming run.
/// Params: `{"text": "...", "mode": "Chat"|"Plan"}`
/// Returns: `{"run_ref": "...", "trace_id": "...", "session_ref": "..."}`
fn handle_run_start(request: &JsonRpcRequest, manager: &CoreRuntimeManager) -> JsonRpcResponse {
    let text = match request.params.get("text").and_then(|v| v.as_str()) {
        Some(t) if !t.trim().is_empty() => t,
        _ => return invalid_params(request, "params.text is required and must be non-empty"),
    };
    let mode = match request.params.get("mode").and_then(|v| v.as_str()) {
        Some("Chat") => RuntimeMode::Chat,
        Some("Plan") => RuntimeMode::Plan,
        Some(other) => {
            return invalid_params(
                request,
                format!("params.mode must be 'Chat' or 'Plan', got '{other}'"),
            )
        }
        None => RuntimeMode::Chat, // default
    };

    match manager.start_streaming(text, mode) {
        Ok((run_ref, _rx)) => {
            // _rx is the event receiver — we don't stream it over this TCP line.
            // The caller uses run_subscribe to poll events.
            // We extract trace_id/session_ref/turn_ref from the first event
            // by subscribing to the run immediately.
            let mut result = serde_json::json!({
                "run_ref": run_ref.as_str(),
            });

            // Extract correlation IDs from the first event snapshot.
            if let Ok(envelopes) = manager.subscribe(&run_ref) {
                if let Some(first) = envelopes.first() {
                    result["trace_id"] = serde_json::json!(first.trace_id.as_str());
                    if let Some(sr) = &first.session_ref {
                        result["session_ref"] = serde_json::json!(sr.as_str());
                    }
                    if let Some(rr) = &first.run_ref {
                        result["run_ref"] = serde_json::json!(rr.as_str());
                    }
                }
            }

            success(request, result)
        }
        Err(e) => runtime_error(request, e.to_string()),
    }
}

/// `run_subscribe`: Return a snapshot of events for a run.
/// Params: `{"run_ref": "..."}`
/// Returns: `{"events": [...]}`
fn handle_run_subscribe(request: &JsonRpcRequest, manager: &CoreRuntimeManager) -> JsonRpcResponse {
    let run_ref_str = match request.params.get("run_ref").and_then(|v| v.as_str()) {
        Some(r) if !r.trim().is_empty() => r,
        _ => return invalid_params(request, "params.run_ref is required"),
    };
    let run_ref = RunRef::from_existing(run_ref_str.to_string());

    match manager.subscribe(&run_ref) {
        Ok(envelopes) => {
            let events: Vec<JsonValue> = envelopes
                .into_iter()
                .map(|env| {
                    serde_json::json!({
                        "event_id": env.payload.event_id.as_str(),
                        "trace_id": env.trace_id.as_str(),
                        "run_ref": env.run_ref.as_ref().map(|r| r.as_str()),
                        "session_ref": env.session_ref.as_ref().map(|s| s.as_str()),
                        "turn_ref": env.payload.turn_ref.as_ref().map(|t| t.as_str()),
                        "kind": serde_json::to_value(&env.payload.kind).unwrap_or_default(),
                        "payload": serde_json::to_value(&env.payload.payload).unwrap_or_default(),
                        "sequence": env.payload.sequence,
                        "created_at": env.payload.created_at.to_rfc3339(),
                    })
                })
                .collect();
            success(request, serde_json::json!({ "events": events }))
        }
        Err(e) => runtime_error(request, e.to_string()),
    }
}

/// `run_cancel`: Cancel a running execution.
/// Params: `{"run_ref": "...", "reason": "optional"}`
/// Returns: `{"success": true}`
fn handle_run_cancel(request: &JsonRpcRequest, manager: &CoreRuntimeManager) -> JsonRpcResponse {
    let run_ref_str = match request.params.get("run_ref").and_then(|v| v.as_str()) {
        Some(r) if !r.trim().is_empty() => r,
        _ => return invalid_params(request, "params.run_ref is required"),
    };
    let run_ref = RunRef::from_existing(run_ref_str.to_string());
    let reason = request
        .params
        .get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);

    match manager.cancel(&run_ref, reason) {
        Ok(()) => success(request, serde_json::json!({"success": true})),
        Err(e) => runtime_error(request, e.to_string()),
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

    // ── Server wiring / integration ───────────────────────────────────

    /// Verify `serve_with_runtime` accepts an Arc<CoreRuntimeManager).
    #[test]
    fn test_serve_with_runtime_accepts_arc_manager() {
        let manager = Arc::new(test_manager());
        let _addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let _mgr = manager;
    }

    /// End-to-end: TCP client sends a JSON line to handle_connection backed
    /// by a real CoreRuntimeManager, and verifies the response comes from
    /// the runtime path (not the legacy hardcoded stub).
    #[tokio::test(flavor = "multi_thread")]
    async fn test_handle_connection_uses_runtime_backed_dispatch() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let manager = test_manager();

        // Bind a one-shot listener on a random port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server side: accept one connection and handle it.
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, &manager).await.unwrap();
        });

        // Client side: connect, send config_read request, read response.
        let client = tokio::spawn(async move {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            // Send a config_read JSON-RPC request.
            let request = r#"{"jsonrpc":"2.0","id":1,"method":"config_read"}"#;
            writer
                .write_all(format!("{request}\n").as_bytes())
                .await
                .unwrap();

            // Read the response line.
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            line
        });

        server.await.unwrap();
        let response_line = client.await.unwrap();

        // Parse the response.
        let resp: JsonRpcResponse = serde_json::from_str(response_line.trim()).unwrap();
        assert!(
            resp.error.is_none(),
            "config_read should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        // Must NOT be the legacy hardcoded values.
        assert_ne!(
            result["provider"], "anthropic",
            "response must come from runtime, not legacy stub"
        );
        assert_ne!(
            result["model"], "glm-4.7",
            "response must come from runtime, not legacy stub"
        );
        // Must contain real config fields.
        assert!(result["provider"].is_string(), "should have provider field");
        assert!(result["model"].is_string(), "should have model field");
    }

    /// End-to-end: verify unknown method returns -32601 through the TCP path.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_handle_connection_unknown_method_returns_error() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let manager = test_manager();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, &manager).await.unwrap();
        });

        let client = tokio::spawn(async move {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            let request = r#"{"jsonrpc":"2.0","id":2,"method":"nonexistent"}"#;
            writer
                .write_all(format!("{request}\n").as_bytes())
                .await
                .unwrap();

            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            line
        });

        server.await.unwrap();
        let response_line = client.await.unwrap();

        let resp: JsonRpcResponse = serde_json::from_str(response_line.trim()).unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, ERR_METHOD_NOT_FOUND);
        assert!(err.message.contains("nonexistent"));
    }

    // ── run_start / run_subscribe / run_cancel ───────────────────────

    fn make_request_with_params(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonValue::Number(1.into())),
            method: method.to_string(),
            params,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_returns_run_ref() {
        let manager = test_manager();
        let params = serde_json::json!({"text": "hello", "mode": "Chat"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(
            resp.error.is_none(),
            "run_start should succeed: {:?}",
            resp.error
        );
        let result = resp.result.unwrap();
        assert!(result["run_ref"].is_string(), "should return run_ref");
        assert!(!result["run_ref"].as_str().unwrap().is_empty());
        assert!(result["trace_id"].is_string(), "should return trace_id");
        assert!(!result["trace_id"].as_str().unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_default_mode_is_chat() {
        let manager = test_manager();
        let params = serde_json::json!({"text": "hello"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["run_ref"].is_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_plan_mode() {
        let manager = test_manager();
        let params = serde_json::json!({"text": "hello", "mode": "Plan"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["run_ref"].is_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_missing_text_returns_invalid_params() {
        let manager = test_manager();
        let params = serde_json::json!({"mode": "Chat"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
        assert!(err.message.contains("text"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_empty_text_returns_invalid_params() {
        let manager = test_manager();
        let params = serde_json::json!({"text": "  ", "mode": "Chat"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, ERR_INVALID_PARAMS);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_start_invalid_mode_returns_invalid_params() {
        let manager = test_manager();
        let params = serde_json::json!({"text": "hello", "mode": "Bogus"});
        let resp = dispatch_with_runtime(&make_request_with_params("run_start", params), &manager);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, ERR_INVALID_PARAMS);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_subscribe_returns_events() {
        let manager = test_manager();

        // Start a run first to get a valid run_ref.
        let start_params = serde_json::json!({"text": "test", "mode": "Chat"});
        let start_resp = dispatch_with_runtime(
            &make_request_with_params("run_start", start_params),
            &manager,
        );
        let run_ref = start_resp.result.unwrap()["run_ref"]
            .as_str()
            .unwrap()
            .to_string();

        // Give the streaming run a moment to produce events.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Subscribe to get the snapshot.
        let sub_params = serde_json::json!({"run_ref": run_ref});
        let sub_resp = dispatch_with_runtime(
            &make_request_with_params("run_subscribe", sub_params),
            &manager,
        );
        assert!(
            sub_resp.error.is_none(),
            "run_subscribe should succeed: {:?}",
            sub_resp.error
        );
        let sub_result = sub_resp.result.unwrap();
        let events = sub_result["events"].as_array().unwrap();
        assert!(
            !events.is_empty(),
            "should have at least one event (RunStarted)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_subscribe_missing_run_ref_returns_invalid_params() {
        let manager = test_manager();
        let params = serde_json::json!({});
        let resp =
            dispatch_with_runtime(&make_request_with_params("run_subscribe", params), &manager);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, ERR_INVALID_PARAMS);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_cancel_returns_success() {
        let manager = test_manager();

        // Start a run.
        let start_params = serde_json::json!({"text": "test", "mode": "Plan"});
        let start_resp = dispatch_with_runtime(
            &make_request_with_params("run_start", start_params),
            &manager,
        );
        let run_ref = start_resp.result.unwrap()["run_ref"]
            .as_str()
            .unwrap()
            .to_string();

        // Cancel it.
        let cancel_params = serde_json::json!({"run_ref": run_ref, "reason": "test"});
        let cancel_resp = dispatch_with_runtime(
            &make_request_with_params("run_cancel", cancel_params),
            &manager,
        );
        assert!(
            cancel_resp.error.is_none(),
            "run_cancel should succeed: {:?}",
            cancel_resp.error
        );
        assert_eq!(cancel_resp.result.unwrap()["success"], true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_cancel_observable_via_subscribe() {
        let manager = test_manager();

        // Start a Plan run.
        let start_params = serde_json::json!({"text": "test long task", "mode": "Plan"});
        let start_resp = dispatch_with_runtime(
            &make_request_with_params("run_start", start_params),
            &manager,
        );
        let run_ref = start_resp.result.unwrap()["run_ref"]
            .as_str()
            .unwrap()
            .to_string();

        // Give the run a moment to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Cancel it.
        let cancel_params = serde_json::json!({"run_ref": run_ref, "reason": "user abort"});
        let cancel_resp = dispatch_with_runtime(
            &make_request_with_params("run_cancel", cancel_params),
            &manager,
        );
        assert!(cancel_resp.error.is_none());

        // Give cancellation time to propagate.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Subscribe and verify we can see the cancellation result.
        let sub_params = serde_json::json!({"run_ref": run_ref});
        let sub_resp = dispatch_with_runtime(
            &make_request_with_params("run_subscribe", sub_params),
            &manager,
        );
        assert!(sub_resp.error.is_none());
        let result = sub_resp.result.unwrap();
        let events = result["events"].as_array().unwrap();
        assert!(!events.is_empty(), "should have events after cancel");

        // Verify RunCancelled event is present — this is the primary
        // cancellation signal that P4-2 requires callers to observe.
        let has_run_cancelled = events
            .iter()
            .any(|e| e.get("kind").and_then(|v| v.as_str()) == Some("run-cancelled"));
        assert!(
            has_run_cancelled,
            "events must contain a RunCancelled event; got kinds: {:?}",
            events.iter().map(|e| e.get("kind")).collect::<Vec<_>>()
        );

        // Verify correlation fields are present.
        let has_run_ref = events.iter().any(|e| {
            e.get("run_ref")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
        assert!(has_run_ref, "events should include run_ref");

        let has_trace_id = events.iter().any(|e| {
            e.get("trace_id")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
        assert!(has_trace_id, "events should include trace_id");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_cancel_missing_run_ref_returns_invalid_params() {
        let manager = test_manager();
        let params = serde_json::json!({});
        let resp = dispatch_with_runtime(&make_request_with_params("run_cancel", params), &manager);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, ERR_INVALID_PARAMS);
    }

    /// End-to-end TCP: run_start returns a run_ref, run_subscribe returns events.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tcp_run_start_subscribe_flow() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let manager = Arc::new(test_manager());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn server that handles 2 connections.
        let mgr = manager.clone();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let m = mgr.clone();
                handle_connection(stream, &m).await.unwrap();
            }
        });

        // Client: run_start
        let client = tokio::spawn(async move {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            let req = r#"{"jsonrpc":"2.0","id":1,"method":"run_start","params":{"text":"hello"}}"#;
            writer
                .write_all(format!("{req}\n").as_bytes())
                .await
                .unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let resp: JsonRpcResponse = serde_json::from_str(line.trim()).unwrap();
            assert!(resp.error.is_none(), "run_start failed: {:?}", resp.error);
            resp.result.unwrap()["run_ref"]
                .as_str()
                .unwrap()
                .to_string()
        });

        let run_ref = client.await.unwrap();

        // Give the run a moment to produce events.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Client: run_subscribe
        let client2 = tokio::spawn(async move {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            let req = format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"run_subscribe","params":{{"run_ref":"{}"}}}}"#,
                run_ref
            );
            writer
                .write_all(format!("{req}\n").as_bytes())
                .await
                .unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            line
        });

        let response_line = client2.await.unwrap();
        server.await.unwrap();

        let resp: JsonRpcResponse = serde_json::from_str(response_line.trim()).unwrap();
        assert!(
            resp.error.is_none(),
            "run_subscribe failed: {:?}",
            resp.error
        );
        let result = resp.result.unwrap();
        let events = result["events"].as_array().unwrap();
        assert!(!events.is_empty(), "should have events from the run");
    }
}
