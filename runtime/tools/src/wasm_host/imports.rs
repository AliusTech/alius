//! WASM-to-host imports for plugin capability access.
//!
//! Provides six host functions registered under the `"alius_host"` namespace:
//! - `read_file` — read a file relative to workspace
//! - `write_file` — write a file relative to workspace
//! - `list_dir` — list directory entries relative to workspace
//! - `env_get` — read an environment variable
//! - `shell` — execute a shell command (must also pass Shell Gate)
//! - `fetch` — HTTPS-only HTTP fetch (deny-by-default, permission-gated)
//!
//! Each import follows the pipeline:
//! 1. Parse WASM memory parameters (JSON)
//! 2. Permission matcher check
//! 3. Domain security primitive (workspace boundary / Shell Gate / env validation)
//! 4. Audit log
//! 5. Execute or return denial

use anyhow::{bail, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::audit::{audit_event, HostAuditSink, TracingAuditSink};
use super::host::{PermissionDecision, ResolvedPluginPermissions};

/// Host state stored in the wasmtime Store.
pub struct WasmHostState {
    pub permissions: ResolvedPluginPermissions,
    pub plugin_id: String,
    pub workspace_root: PathBuf,
    pub audit: Arc<dyn HostAuditSink>,
    pub trace_id: String,
    pub bypass_permissions: bool,
}

impl WasmHostState {
    pub fn new(
        permissions: ResolvedPluginPermissions,
        plugin_id: String,
        workspace_root: PathBuf,
        trace_id: String,
    ) -> Self {
        Self {
            permissions,
            plugin_id,
            workspace_root,
            audit: Arc::new(TracingAuditSink),
            trace_id,
            bypass_permissions: false,
        }
    }

    pub fn with_audit(mut self, audit: Arc<dyn HostAuditSink>) -> Self {
        self.audit = audit;
        self
    }

    pub fn with_bypass_permissions(mut self, bypass_permissions: bool) -> Self {
        self.bypass_permissions = bypass_permissions;
        self
    }

    fn emit_audit(&self, action: &str, target: &str, allowed: bool, reason: &str) {
        let event = audit_event(
            &self.trace_id,
            &self.plugin_id,
            action,
            target,
            allowed,
            reason,
        );
        self.audit.emit(event);
    }
}

/// JSON response helpers.
fn ok_response(data: serde_json::Value) -> String {
    serde_json::to_string(&json!({"ok": true, "data": data})).unwrap_or_default()
}

fn err_response(code: &str, message: &str) -> String {
    serde_json::to_string(&json!({"ok": false, "error": message, "code": code})).unwrap_or_default()
}

fn bypass_path(path: &Path, workspace_root: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

/// Write a result JSON string into WASM memory and return packed (ptr, len) as i64.
/// Uses a fixed offset (4096) as a simple bump region, well above input data
/// (which lives at offset 0-256 max) and within a single WASM page (64KB).
fn write_json_result(
    memory: &wasmtime::Memory,
    caller: &mut wasmtime::Caller<'_, WasmHostState>,
    result: &str,
) -> i64 {
    let bytes = result.as_bytes();
    let offset: usize = 4096;
    let mem = match memory.data_mut(&mut *caller) {
        m if offset + 4 + bytes.len() <= m.len() => m,
        _ => return -1i64,
    };
    let len_bytes = (bytes.len() as u32).to_le_bytes();
    mem[offset..offset + 4].copy_from_slice(&len_bytes);
    mem[offset + 4..offset + 4 + bytes.len()].copy_from_slice(bytes);
    ((offset as i64) << 32) | ((4 + bytes.len()) as i64)
}

/// Extract a string from WASM memory given ptr/len.
fn extract_string(
    memory: &wasmtime::Memory,
    caller: &wasmtime::Caller<'_, WasmHostState>,
    ptr: i32,
    len: i32,
) -> Result<String> {
    let ptr = ptr as usize;
    let len = len as usize;
    let data = memory.data(caller);
    if ptr + len > data.len() {
        bail!("WASM memory out of bounds");
    }
    Ok(std::str::from_utf8(&data[ptr..ptr + len])?.to_string())
}

/// Validate a fetch URL against permission and protocol rules.
///
/// Returns Ok(()) if the URL is allowed, Err(reason) if denied.
/// This is extracted from the fetch import closure for testability.
fn validate_fetch_url(url: &str, permissions: &ResolvedPluginPermissions) -> Result<(), String> {
    // HTTPS-only enforcement (check first, before permission matching)
    if !url.starts_with("https://") {
        return Err("only https:// URLs are allowed".to_string());
    }

    // Permission check
    let decision = permissions.check_network(url);
    if !decision.is_allowed() {
        let reason = match &decision {
            PermissionDecision::Deny { reason } => reason.clone(),
            _ => "denied".to_string(),
        };
        return Err(reason);
    }

    Ok(())
}

const FETCH_MAX_BYTES: usize = 1_048_576; // 1 MB

/// Execute an HTTPS fetch with timeout and size limits.
///
/// Returns the response JSON on success, or an error string on failure.
/// This is extracted from the fetch import closure for testability.
async fn execute_fetch(url: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let status = resp.status().as_u16();
    let content_length = resp.content_length().unwrap_or(0);
    if content_length > FETCH_MAX_BYTES as u64 {
        return Err(format!(
            "response too large: {} bytes (max 1MB)",
            content_length
        ));
    }

    let body = resp.text().await.map_err(|e| format!("body read: {}", e))?;

    if body.len() > FETCH_MAX_BYTES {
        return Err("response body exceeds 1MB limit".to_string());
    }

    Ok(serde_json::json!({
        "status": status,
        "content_type": "text/plain",
        "body": body,
    }))
}

/// Build a wasmtime Linker with all host imports registered.
pub fn build_linker(engine: &wasmtime::Engine) -> Result<wasmtime::Linker<WasmHostState>> {
    let mut linker = wasmtime::Linker::<WasmHostState>::new(engine);

    // read_file(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "read_file",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let path_str = match params["path"].as_str() {
                Some(p) => p.to_string(),
                None => return -1i64,
            };
            let path = Path::new(&path_str);

            let state = caller.data();
            let full_path = if state.bypass_permissions {
                bypass_path(path, &state.workspace_root)
            } else {
                let decision =
                    state
                        .permissions
                        .check_filesystem("read", path, &state.workspace_root);
                if !decision.is_allowed() {
                    let reason = match &decision {
                        PermissionDecision::Deny { reason } => reason.clone(),
                        _ => "denied".to_string(),
                    };
                    state.emit_audit("read_file", &path_str, false, &reason);
                    return write_json_result(
                        &memory,
                        &mut caller,
                        &err_response("permission_denied", &reason),
                    );
                }

                decision
                    .resolved_path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| state.workspace_root.join(path))
            };
            let result = match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    state.emit_audit("read_file", &path_str, true, "ok");
                    ok_response(serde_json::Value::String(content))
                }
                Err(e) => {
                    state.emit_audit("read_file", &path_str, true, "ok");
                    err_response("io_error", &e.to_string())
                }
            };
            write_json_result(&memory, &mut caller, &result)
        },
    )?;

    // write_file(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "write_file",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let path_str = match params["path"].as_str() {
                Some(p) => p.to_string(),
                None => return -1i64,
            };
            let content = match params["content"].as_str() {
                Some(c) => c.to_string(),
                None => return -1i64,
            };
            let path = Path::new(&path_str);

            let state = caller.data();
            let full_path = if state.bypass_permissions {
                bypass_path(path, &state.workspace_root)
            } else {
                let decision =
                    state
                        .permissions
                        .check_filesystem("write", path, &state.workspace_root);
                if !decision.is_allowed() {
                    let reason = match &decision {
                        PermissionDecision::Deny { reason } => reason.clone(),
                        _ => "denied".to_string(),
                    };
                    state.emit_audit("write_file", &path_str, false, &reason);
                    return write_json_result(
                        &memory,
                        &mut caller,
                        &err_response("permission_denied", &reason),
                    );
                }

                decision
                    .resolved_path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| state.workspace_root.join(path))
            };
            if let Some(parent) = full_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let result = match std::fs::write(&full_path, &content) {
                Ok(()) => {
                    state.emit_audit("write_file", &path_str, true, "ok");
                    ok_response(json!(null))
                }
                Err(e) => {
                    state.emit_audit("write_file", &path_str, true, "ok");
                    err_response("io_error", &e.to_string())
                }
            };
            write_json_result(&memory, &mut caller, &result)
        },
    )?;

    // list_dir(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "list_dir",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let path_str = match params["path"].as_str() {
                Some(p) => p.to_string(),
                None => return -1i64,
            };
            let path = Path::new(&path_str);

            let state = caller.data();
            let full_path = if state.bypass_permissions {
                bypass_path(path, &state.workspace_root)
            } else {
                let decision =
                    state
                        .permissions
                        .check_filesystem("list", path, &state.workspace_root);
                if !decision.is_allowed() {
                    let reason = match &decision {
                        PermissionDecision::Deny { reason } => reason.clone(),
                        _ => "denied".to_string(),
                    };
                    state.emit_audit("list_dir", &path_str, false, &reason);
                    return write_json_result(
                        &memory,
                        &mut caller,
                        &err_response("permission_denied", &reason),
                    );
                }

                decision
                    .resolved_path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| state.workspace_root.join(path))
            };
            let result = match std::fs::read_dir(&full_path) {
                Ok(entries) => {
                    let names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    state.emit_audit("list_dir", &path_str, true, "ok");
                    ok_response(json!(names))
                }
                Err(e) => {
                    state.emit_audit("list_dir", &path_str, true, "ok");
                    err_response("io_error", &e.to_string())
                }
            };
            write_json_result(&memory, &mut caller, &result)
        },
    )?;

    // env_get(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "env_get",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let name = match params["name"].as_str() {
                Some(n) => n.to_string(),
                None => return -1i64,
            };

            let state = caller.data();
            if !state.bypass_permissions {
                let decision = state.permissions.check_env(&name);
                if !decision.is_allowed() {
                    let reason = match &decision {
                        PermissionDecision::Deny { reason } => reason.clone(),
                        _ => "denied".to_string(),
                    };
                    state.emit_audit("env_get", &name, false, &reason);
                    return write_json_result(
                        &memory,
                        &mut caller,
                        &err_response("permission_denied", &reason),
                    );
                }
            }

            // SECURITY: env VALUE is never logged in audit
            let result = match std::env::var(&name) {
                Ok(value) => {
                    state.emit_audit("env_get", &name, true, "ok");
                    ok_response(serde_json::Value::String(value))
                }
                Err(std::env::VarError::NotPresent) => {
                    state.emit_audit("env_get", &name, true, "ok");
                    ok_response(json!(null))
                }
                Err(e) => {
                    state.emit_audit("env_get", &name, true, "ok");
                    err_response("env_error", &e.to_string())
                }
            };
            write_json_result(&memory, &mut caller, &result)
        },
    )?;

    // shell(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "shell",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let command = match params["command"].as_str() {
                Some(c) => c.to_string(),
                None => return -1i64,
            };

            let state = caller.data();
            let base_cmd = command.split_whitespace().next().unwrap_or(&command);

            if !state.bypass_permissions {
                let decision = state.permissions.check_shell(&command);
                if !decision.is_allowed() {
                    let reason = match &decision {
                        PermissionDecision::Deny { reason } => reason.clone(),
                        _ => "denied".to_string(),
                    };
                    state.emit_audit("shell", base_cmd, false, &reason);
                    return write_json_result(
                        &memory,
                        &mut caller,
                        &err_response("permission_denied", &reason),
                    );
                }

                let shell_request = crate::shell_gate::ShellCommandRequest {
                    command: command.clone(),
                    args: command.split_whitespace().map(String::from).collect(),
                    cwd: state.workspace_root.clone(),
                    origin: crate::shell_gate::ShellOrigin::WasmPlugin,
                    workspace_root: state.workspace_root.clone(),
                };
                let shell_config = crate::shell_gate::ShellGateConfig::default();
                let (gate_decision, _risk) =
                    crate::shell_gate::authorizer::authorize(&shell_request, &shell_config);
                match gate_decision {
                    crate::shell_gate::ShellGateDecision::Allow => {}
                    crate::shell_gate::ShellGateDecision::Deny { reason } => {
                        state.emit_audit(
                            "shell",
                            base_cmd,
                            false,
                            &format!("shell_gate_deny: {reason}"),
                        );
                        return write_json_result(
                            &memory,
                            &mut caller,
                            &err_response("shell_gate_denied", &reason),
                        );
                    }
                    crate::shell_gate::ShellGateDecision::ApprovalRequired { reason } => {
                        state.emit_audit(
                            "shell",
                            base_cmd,
                            false,
                            &format!("shell_gate_approval_required: {reason}"),
                        );
                        return write_json_result(
                            &memory,
                            &mut caller,
                            &err_response(
                                "shell_gate_denied",
                                &format!("approval required but no confirmation channel: {reason}"),
                            ),
                        );
                    }
                }
            }

            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output();

            let result = match output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let exit_code = output.status.code().unwrap_or(-1);
                    // SECURITY: stdout/stderr NOT logged in audit
                    state.emit_audit("shell", base_cmd, true, "ok");
                    ok_response(json!({"stdout": stdout, "stderr": stderr, "exit_code": exit_code}))
                }
                Err(e) => {
                    state.emit_audit("shell", base_cmd, true, "ok");
                    err_response("exec_error", &e.to_string())
                }
            };
            write_json_result(&memory, &mut caller, &result)
        },
    )?;

    // fetch(ptr: i32, len: i32) -> i64
    linker.func_wrap(
        "alius_host",
        "fetch",
        |mut caller: wasmtime::Caller<'_, WasmHostState>, ptr: i32, len: i32| -> i64 {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1i64,
            };

            let json_str = match extract_string(&memory, &caller, ptr, len) {
                Ok(s) => s,
                Err(_) => return -1i64,
            };

            let params: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => return -1i64,
            };

            let url = match params["url"].as_str() {
                Some(u) => u.to_string(),
                None => return -1i64,
            };

            let state = caller.data();

            if !state.bypass_permissions {
                if let Err(reason) = validate_fetch_url(&url, &state.permissions) {
                    state.emit_audit("fetch", &url, false, &reason);
                    let code = if reason.contains("not permitted") || reason.contains("no network")
                    {
                        "permission_denied"
                    } else {
                        "denied"
                    };
                    return write_json_result(&memory, &mut caller, &err_response(code, &reason));
                }
            }

            // Execute HTTP request synchronously. We cannot use
            // Handle::block_on() here because WASM host functions may be called
            // from within an existing tokio runtime (where block_on panics).
            // Instead, spawn a dedicated thread with its own runtime.
            let url_clone = url.clone();
            let result = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("runtime: {}", e))?;
                rt.block_on(execute_fetch(&url_clone))
            })
            .join()
            .unwrap_or_else(|_| Err("fetch thread panicked".to_string()));

            match result {
                Ok(json) => {
                    let json_str = json.to_string();
                    state.emit_audit("fetch", &url, true, "ok");
                    write_json_result(&memory, &mut caller, &json_str)
                }
                Err(e) => {
                    state.emit_audit("fetch", &url, false, &e);
                    let err = err_response("fetch_error", &e);
                    write_json_result(&memory, &mut caller, &err)
                }
            }
        },
    )?;

    Ok(linker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct RecordingSink {
        events: Arc<Mutex<Vec<super::super::audit::HostAuditEvent>>>,
    }

    impl RecordingSink {
        fn new() -> (Self, Arc<Mutex<Vec<super::super::audit::HostAuditEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: Arc::clone(&events),
                },
                events,
            )
        }
    }

    impl HostAuditSink for RecordingSink {
        fn emit(&self, event: super::super::audit::HostAuditEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn test_build_linker_succeeds() {
        let engine = wasmtime::Engine::default();
        let linker = build_linker(&engine);
        assert!(linker.is_ok());
    }

    #[test]
    fn test_err_response_format() {
        let resp = err_response("permission_denied", "no filesystem permissions declared");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], "permission_denied");
        assert!(v["error"].as_str().unwrap().contains("filesystem"));
    }

    #[test]
    fn test_ok_response_format() {
        let resp = ok_response(json!("test data"));
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["data"], "test data");
    }

    // ===== Integration tests with real WASM modules =====

    /// Helper: create a WAT module that imports all 6 host functions and
    /// exports a `call_read_file` function that writes JSON params to memory,
    /// calls the host import, and returns the packed result.
    fn make_test_wasm() -> Vec<u8> {
        // Minimal WAT that imports read_file from alius_host and can call it.
        // The module:
        // 1. Has 1 page of memory (64KB) exported as "memory"
        // 2. Imports read_file(ptr, len) -> i64 from "alius_host"
        // 3. Exports call_read_file() -> i64 which writes JSON to offset 0
        //    and calls the import
        wat::parse_str(
            r#"
            (module
                (import "alius_host" "read_file" (func $read_file (param i32 i32) (result i64)))
                (import "alius_host" "write_file" (func $write_file (param i32 i32) (result i64)))
                (import "alius_host" "list_dir" (func $list_dir (param i32 i32) (result i64)))
                (import "alius_host" "env_get" (func $env_get (param i32 i32) (result i64)))
                (import "alius_host" "shell" (func $shell (param i32 i32) (result i64)))
                (import "alius_host" "fetch" (func $fetch (param i32 i32) (result i64)))
                (memory (export "memory") 1)

                ;; Helper: write bytes at offset, return (ptr, len) packed as i64
                (func $call_read_file (result i64)
                    ;; Write {"path":"data/hello.txt"} at offset 0
                    (i32.store8 (i32.const 0) (i32.const 123))   ;; {
                    (i32.store8 (i32.const 1) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 2) (i32.const 112))   ;; p
                    (i32.store8 (i32.const 3) (i32.const 97))    ;; a
                    (i32.store8 (i32.const 4) (i32.const 116))   ;; t
                    (i32.store8 (i32.const 5) (i32.const 104))   ;; h
                    (i32.store8 (i32.const 6) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 7) (i32.const 58))    ;; :
                    (i32.store8 (i32.const 8) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 9) (i32.const 100))   ;; d
                    (i32.store8 (i32.const 10) (i32.const 97))   ;; a
                    (i32.store8 (i32.const 11) (i32.const 116))  ;; t
                    (i32.store8 (i32.const 12) (i32.const 97))   ;; a
                    (i32.store8 (i32.const 13) (i32.const 47))   ;; /
                    (i32.store8 (i32.const 14) (i32.const 104))  ;; h
                    (i32.store8 (i32.const 15) (i32.const 101))  ;; e
                    (i32.store8 (i32.const 16) (i32.const 108))  ;; l
                    (i32.store8 (i32.const 17) (i32.const 108))  ;; l
                    (i32.store8 (i32.const 18) (i32.const 111))  ;; o
                    (i32.store8 (i32.const 19) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 20) (i32.const 116))  ;; t
                    (i32.store8 (i32.const 21) (i32.const 120))  ;; x
                    (i32.store8 (i32.const 22) (i32.const 116))  ;; t
                    (i32.store8 (i32.const 23) (i32.const 34))   ;; "
                    (i32.store8 (i32.const 24) (i32.const 125))  ;; }
                    ;; Call read_file(0, 25)
                    (call $read_file (i32.const 0) (i32.const 25))
                )
                (export "call_read_file" (func $call_read_file))
            )
        "#,
        )
        .unwrap()
    }

    fn make_test_workspace() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("alius_imports_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("data")).unwrap();
        std::fs::write(dir.join("data/hello.txt"), "hello world").unwrap();
        dir
    }

    fn cleanup_workspace(ws: &Path) {
        let _ = std::fs::remove_dir_all(ws);
    }

    #[test]
    fn test_host_import_read_file_allowed() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();

        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:data".to_string()],
            ..Default::default()
        };
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-1".to_string(),
        )
        .with_audit(Arc::new(sink));

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_binary(&engine, &make_test_wasm()).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_read_file")
            .unwrap();

        let result = call_fn.call(&mut store, ()).unwrap();

        // High 32 bits = ptr, low 32 bits = len
        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;
        assert!(len > 0, "should have written a result");

        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        // Skip 4-byte length prefix
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["data"], "hello world");

        // Verify audit was emitted
        let audit_events = events.lock().unwrap();
        assert_eq!(audit_events.len(), 1);
        assert_eq!(audit_events[0].action, "read_file");
        assert!(audit_events[0].allowed);
        assert_eq!(audit_events[0].plugin_id, "test-plugin");

        cleanup_workspace(&ws);
    }

    #[test]
    fn test_host_import_read_file_denied_no_permissions() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();

        let perms = ResolvedPluginPermissions::default(); // no permissions
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-2".to_string(),
        )
        .with_audit(Arc::new(sink));

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_binary(&engine, &make_test_wasm()).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_read_file")
            .unwrap();

        let result = call_fn.call(&mut store, ()).unwrap();

        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;
        assert!(len > 0);

        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], "permission_denied");

        // Verify audit was emitted with denied
        let audit_events = events.lock().unwrap();
        assert_eq!(audit_events.len(), 1);
        assert!(!audit_events[0].allowed);
        assert!(audit_events[0].reason.contains("no filesystem"));

        cleanup_workspace(&ws);
    }

    #[test]
    fn test_host_import_read_file_path_escape_denied() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();

        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:data".to_string()],
            ..Default::default()
        };
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-3".to_string(),
        )
        .with_audit(Arc::new(sink));

        // Create a WAT module that tries to read outside workspace
        let wasm = wat::parse_str(
            r#"
            (module
                (import "alius_host" "read_file" (func $read_file (param i32 i32) (result i64)))
                (import "alius_host" "write_file" (func $write_file (param i32 i32) (result i64)))
                (import "alius_host" "list_dir" (func $list_dir (param i32 i32) (result i64)))
                (import "alius_host" "env_get" (func $env_get (param i32 i32) (result i64)))
                (import "alius_host" "shell" (func $shell (param i32 i32) (result i64)))
                (import "alius_host" "fetch" (func $fetch (param i32 i32) (result i64)))
                (memory (export "memory") 1)
                (func $call_escape (result i64)
                    ;; Write {"path":"../../../etc/passwd"} at offset 0
                    ;; path = "../../../etc/passwd" = 20 chars
                    ;; JSON = {"path":"../../../etc/passwd"} = 30 chars
                    (i32.store8 (i32.const 0) (i32.const 123))   ;; {
                    (i32.store8 (i32.const 1) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 2) (i32.const 112))   ;; p
                    (i32.store8 (i32.const 3) (i32.const 97))    ;; a
                    (i32.store8 (i32.const 4) (i32.const 116))   ;; t
                    (i32.store8 (i32.const 5) (i32.const 104))   ;; h
                    (i32.store8 (i32.const 6) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 7) (i32.const 58))    ;; :
                    (i32.store8 (i32.const 8) (i32.const 34))    ;; "
                    (i32.store8 (i32.const 9) (i32.const 46))    ;; .
                    (i32.store8 (i32.const 10) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 11) (i32.const 47))   ;; /
                    (i32.store8 (i32.const 12) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 13) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 14) (i32.const 47))   ;; /
                    (i32.store8 (i32.const 15) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 16) (i32.const 46))   ;; .
                    (i32.store8 (i32.const 17) (i32.const 47))   ;; /
                    (i32.store8 (i32.const 18) (i32.const 101))  ;; e
                    (i32.store8 (i32.const 19) (i32.const 116))  ;; t
                    (i32.store8 (i32.const 20) (i32.const 99))   ;; c
                    (i32.store8 (i32.const 21) (i32.const 47))   ;; /
                    (i32.store8 (i32.const 22) (i32.const 112))  ;; p
                    (i32.store8 (i32.const 23) (i32.const 97))   ;; a
                    (i32.store8 (i32.const 24) (i32.const 115))  ;; s
                    (i32.store8 (i32.const 25) (i32.const 115))  ;; s
                    (i32.store8 (i32.const 26) (i32.const 119))  ;; w
                    (i32.store8 (i32.const 27) (i32.const 100))  ;; d
                    (i32.store8 (i32.const 28) (i32.const 34))   ;; "
                    (i32.store8 (i32.const 29) (i32.const 125))  ;; }
                    (call $read_file (i32.const 0) (i32.const 30))
                )
                (export "call_escape" (func $call_escape))
            )
        "#,
        )
        .unwrap();

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_binary(&engine, &wasm).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_escape")
            .unwrap();

        let result = call_fn.call(&mut store, ()).unwrap();
        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;

        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], "permission_denied");

        let audit_events = events.lock().unwrap();
        assert_eq!(audit_events.len(), 1);
        assert!(!audit_events[0].allowed);

        cleanup_workspace(&ws);
    }

    #[test]
    fn test_shell_gate_blocks_dangerous_command() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();

        // Permission allows shell exec:readonly — which includes `rm` if it were
        // in the readonly set. But even if permission allows, Shell Gate blocks
        // high-risk/critical commands.
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-shell-gate".to_string(),
        )
        .with_audit(Arc::new(sink));

        // Create WAT module that calls shell with "rm -rf /"
        let wasm = wat::parse_str(
            r#"
            (module
                (import "alius_host" "read_file" (func $read_file (param i32 i32) (result i64)))
                (import "alius_host" "write_file" (func $write_file (param i32 i32) (result i64)))
                (import "alius_host" "list_dir" (func $list_dir (param i32 i32) (result i64)))
                (import "alius_host" "env_get" (func $env_get (param i32 i32) (result i64)))
                (import "alius_host" "shell" (func $shell (param i32 i32) (result i64)))
                (import "alius_host" "fetch" (func $fetch (param i32 i32) (result i64)))
                (memory (export "memory") 1)
                (func $call_rm (result i64)
                    ;; Write {"command":"rm -rf /"} at offset 0
                    (i32.store8 (i32.const 0) (i32.const 123))  ;; {
                    (i32.store8 (i32.const 1) (i32.const 34))   ;; "
                    (i32.store8 (i32.const 2) (i32.const 99))   ;; c
                    (i32.store8 (i32.const 3) (i32.const 111))  ;; o
                    (i32.store8 (i32.const 4) (i32.const 109))  ;; m
                    (i32.store8 (i32.const 5) (i32.const 109))  ;; m
                    (i32.store8 (i32.const 6) (i32.const 97))   ;; a
                    (i32.store8 (i32.const 7) (i32.const 110))  ;; n
                    (i32.store8 (i32.const 8) (i32.const 100))  ;; d
                    (i32.store8 (i32.const 9) (i32.const 34))   ;; "
                    (i32.store8 (i32.const 10) (i32.const 58))  ;; :
                    (i32.store8 (i32.const 11) (i32.const 34))  ;; "
                    (i32.store8 (i32.const 12) (i32.const 114)) ;; r
                    (i32.store8 (i32.const 13) (i32.const 109)) ;; m
                    (i32.store8 (i32.const 14) (i32.const 32))  ;; space
                    (i32.store8 (i32.const 15) (i32.const 45))  ;; -
                    (i32.store8 (i32.const 16) (i32.const 114)) ;; r
                    (i32.store8 (i32.const 17) (i32.const 102)) ;; f
                    (i32.store8 (i32.const 18) (i32.const 32))  ;; space
                    (i32.store8 (i32.const 19) (i32.const 47))  ;; /
                    (i32.store8 (i32.const 20) (i32.const 34))  ;; "
                    (i32.store8 (i32.const 21) (i32.const 125)) ;; }
                    (call $shell (i32.const 0) (i32.const 22))
                )
                (export "call_rm" (func $call_rm))
            )
        "#,
        )
        .unwrap();

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_binary(&engine, &wasm).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_rm")
            .unwrap();

        let result = call_fn.call(&mut store, ()).unwrap();
        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;

        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();

        // Shell Gate should block rm -rf (Critical risk or permission denied)
        assert_eq!(v["ok"], json!(false), "Shell Gate should block rm -rf /");

        // Verify audit was emitted with denial
        let audit_events = events.lock().unwrap();
        assert!(!audit_events.is_empty());
        assert!(!audit_events[0].allowed);

        cleanup_workspace(&ws);
    }

    // ===== Fetch validation tests =====

    #[test]
    fn test_fetch_no_network_permission_denied() {
        let perms = ResolvedPluginPermissions::default();
        let result = validate_fetch_url("https://api.example.com/data", &perms);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no network permissions"));
    }

    #[test]
    fn test_fetch_http_url_denied() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let result = validate_fetch_url("http://api.example.com/data", &perms);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only https://"));
    }

    #[test]
    fn test_fetch_similar_domain_denied() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let result = validate_fetch_url("https://api.example.com.evil.com/data", &perms);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not permitted"));
    }

    #[test]
    fn test_fetch_allowed_url_passes_validation() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let result = validate_fetch_url("https://api.example.com/data", &perms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fetch_exact_url_passes_validation() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com/v1".to_string()],
            ..Default::default()
        };
        let result = validate_fetch_url("https://api.example.com/v1", &perms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fetch_undeclared_domain_denied() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let result = validate_fetch_url("https://other.example.com/data", &perms);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not permitted"));
    }

    // ===== execute_fetch execution-level tests =====

    /// Start a local TCP listener that responds with a canned HTTP response.
    /// Returns the URL to connect to (http://127.0.0.1:{port}).
    /// The returned JoinHandle must be kept alive to prevent the server from
    /// being dropped before the test completes.
    async fn start_test_server(response: String) -> Option<(tokio::task::JoinHandle<()>, String)> {
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(err) => panic!("failed to bind test server: {err}"),
        };
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}", port);

        let handle = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        // Give the spawned task a moment to start accepting.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Some((handle, url))
    }

    #[tokio::test]
    async fn test_execute_fetch_success() {
        let body = r#"{"status":"ok"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let Some((_handle, url)) = start_test_server(response).await else {
            return;
        };

        let result = execute_fetch(&url).await;
        assert!(result.is_ok(), "Expected success — got: {:?}", result.err());
        let json = result.unwrap();
        assert_eq!(json["status"], 200);
        assert_eq!(json["body"].as_str().unwrap(), body);
    }

    #[tokio::test]
    async fn test_execute_fetch_server_error() {
        let body = "Internal Server Error";
        let response = format!(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let Some((_handle, url)) = start_test_server(response).await else {
            return;
        };

        let result = execute_fetch(&url).await;
        // execute_fetch returns Ok with the status code, not an Err
        assert!(result.is_ok());
        let json = result.unwrap();
        assert_eq!(json["status"], 500);
    }

    #[tokio::test]
    async fn test_execute_fetch_oversized_content_length() {
        // Respond with Content-Length exceeding 1MB
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            FETCH_MAX_BYTES + 1
        );
        let Some((_handle, url)) = start_test_server(response).await else {
            return;
        };

        let result = execute_fetch(&url).await;
        assert!(result.is_err(), "Should reject oversized response");
        assert!(result.unwrap_err().contains("too large"));
    }

    #[tokio::test]
    async fn test_execute_fetch_connection_refused() {
        // Use a non-routable port (nothing listening)
        let result = execute_fetch("http://127.0.0.1:1").await;
        assert!(result.is_err(), "Should fail on connection refused");
    }

    #[tokio::test]
    async fn test_execute_fetch_timeout() {
        // Start a server that accepts but never responds (triggers timeout)
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("failed to bind test server: {err}"),
        };
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}", port);

        let _handle = tokio::spawn(async move {
            // Accept the connection but never send a response
            if let Ok((_stream, _)) = listener.accept().await {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = execute_fetch(&url).await;
        assert!(result.is_err(), "Should fail on timeout");
        let err = result.unwrap_err();
        assert!(
            err.contains("request failed") || err.contains("timeout") || err.contains("timed out"),
            "Error should indicate timeout — got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_execute_fetch_oversized_body() {
        // Start a server that returns a body exceeding 1MB
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("failed to bind test server: {err}"),
        };
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}", port);

        let _handle = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                // Return a body larger than FETCH_MAX_BYTES (1MB)
                let big_body = "x".repeat(FETCH_MAX_BYTES + 100);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    big_body.len(),
                    big_body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = execute_fetch(&url).await;
        assert!(result.is_err(), "Should reject oversized body");
        assert!(
            result.unwrap_err().contains("1MB"),
            "Error should mention 1MB limit"
        );
    }

    // ===== Fetch host import integration tests =====

    /// Helper: WAT module that imports all 6 host functions and exports
    /// `call_fetch` which calls the fetch import with a JSON url parameter
    /// written at memory offset 0.
    fn make_fetch_wasm(url_json: &str) -> Vec<u8> {
        let mut wat = String::from(
            r#"
            (module
                (import "alius_host" "read_file" (func $read_file (param i32 i32) (result i64)))
                (import "alius_host" "write_file" (func $write_file (param i32 i32) (result i64)))
                (import "alius_host" "list_dir" (func $list_dir (param i32 i32) (result i64)))
                (import "alius_host" "env_get" (func $env_get (param i32 i32) (result i64)))
                (import "alius_host" "shell" (func $shell (param i32 i32) (result i64)))
                (import "alius_host" "fetch" (func $fetch (param i32 i32) (result i64)))
                (memory (export "memory") 1)
                (func $call_fetch (result i64)
            "#,
        );
        for (i, b) in url_json.bytes().enumerate() {
            wat.push_str(&format!(
                "                    (i32.store8 (i32.const {}) (i32.const {}))\n",
                i, b
            ));
        }
        wat.push_str(&format!(
            "                    (call $fetch (i32.const 0) (i32.const {}))\n",
            url_json.len()
        ));
        wat.push_str(
            r#"
                )
                (export "call_fetch" (func $call_fetch))
            )
        "#,
        );
        wat::parse_str(&wat).unwrap()
    }

    /// Execute the fetch WASM import and return the parsed JSON result.
    fn execute_fetch_wasm(
        permissions: ResolvedPluginPermissions,
        url_json: &str,
    ) -> serde_json::Value {
        let ws = make_test_workspace();
        let (sink, _events) = RecordingSink::new();
        let state = WasmHostState::new(
            permissions,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-fetch".to_string(),
        )
        .with_audit(Arc::new(sink));

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_binary(&engine, &make_fetch_wasm(url_json)).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_fetch")
            .unwrap();
        let result = call_fn.call(&mut store, ()).unwrap();

        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;
        assert!(len > 0, "fetch should return a result");

        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();

        cleanup_workspace(&ws);
        v
    }

    #[test]
    fn test_fetch_import_rejects_http_url() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:http://example.com".to_string()],
            ..Default::default()
        };
        let url_json = r#"{"url":"http://example.com/data"}"#;
        let v = execute_fetch_wasm(perms, url_json);
        assert_eq!(v["ok"], json!(false), "HTTP URL should be rejected");
        assert_eq!(v["code"], "denied");
        assert!(v["error"].as_str().unwrap().contains("only https://"));
    }

    #[test]
    fn test_fetch_import_rejects_no_permission() {
        let perms = ResolvedPluginPermissions::default(); // no network perms
        let url_json = r#"{"url":"https://example.com/data"}"#;
        let v = execute_fetch_wasm(perms, url_json);
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], "permission_denied");
    }

    #[test]
    fn test_fetch_import_rejects_undeclared_domain() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let url_json = r#"{"url":"https://evil.com/steal"}"#;
        let v = execute_fetch_wasm(perms, url_json);
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], "permission_denied");
    }

    // These tests use #[tokio::test] because the fetch WASM host import calls
    // tokio::runtime::Handle::current().block_on() internally, which requires
    // an active tokio runtime.

    #[tokio::test]
    async fn test_fetch_import_allowed_url_attempts_request() {
        // URL passes validation (HTTPS + allowed domain), but the actual HTTP
        // request will fail because the host doesn't exist. This verifies the
        // full pipeline: permission check → HTTPS enforcement → HTTP execution.
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let url_json = r#"{"url":"https://api.example.com/data"}"#;
        let v = execute_fetch_wasm(perms, url_json);
        // Should NOT be a permission/protocol rejection
        assert_ne!(v["code"], json!("permission_denied"));
        assert_ne!(v["code"], json!("denied"));
        // Will be a fetch_error because the host doesn't resolve
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["code"], json!("fetch_error"));
    }

    #[tokio::test]
    async fn test_fetch_import_audit_on_allowed_url() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-fetch-audit".to_string(),
        )
        .with_audit(Arc::new(sink));

        let engine = wasmtime::Engine::default();
        let url_json = r#"{"url":"https://api.example.com/data"}"#;
        let module = wasmtime::Module::from_binary(&engine, &make_fetch_wasm(url_json)).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_fetch")
            .unwrap();
        let _result = call_fn.call(&mut store, ()).unwrap();

        // Verify audit was emitted (fetch attempt logged even on network failure)
        let audit_events = events.lock().unwrap();
        assert!(!audit_events.is_empty(), "fetch should emit an audit event");
        assert_eq!(audit_events[0].action, "fetch");
        assert_eq!(audit_events[0].target, "https://api.example.com/data");

        cleanup_workspace(&ws);
    }

    #[tokio::test]
    async fn test_fetch_import_audit_on_network_error() {
        let ws = make_test_workspace();
        let (sink, events) = RecordingSink::new();
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let state = WasmHostState::new(
            perms,
            "test-plugin".to_string(),
            ws.clone(),
            "tr-fetch-error-audit".to_string(),
        )
        .with_audit(Arc::new(sink));

        let engine = wasmtime::Engine::default();
        let url_json = r#"{"url":"https://api.example.com/data"}"#;
        let module = wasmtime::Module::from_binary(&engine, &make_fetch_wasm(url_json)).unwrap();
        let linker = build_linker(&engine).unwrap();
        let mut store = wasmtime::Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let call_fn = instance
            .get_typed_func::<(), i64>(&mut store, "call_fetch")
            .unwrap();
        let result = call_fn.call(&mut store, ()).unwrap();

        // Parse the result — should be an error
        let ptr = (result >> 32) as usize;
        let len = (result & 0xFFFFFFFF) as usize;
        let memory = instance.get_memory(&mut store, "memory").unwrap();
        let data = memory.data(&store);
        let json_bytes = &data[ptr..ptr + len];
        let json_str = std::str::from_utf8(&json_bytes[4..]).unwrap();
        let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(v["ok"], json!(false), "Network error should return error");
        assert_eq!(v["code"], "fetch_error");

        // Verify audit was emitted with allowed=false (network failure is logged as denied)
        let audit_events = events.lock().unwrap();
        assert!(
            audit_events
                .iter()
                .any(|e| e.action == "fetch" && !e.allowed),
            "Network error should emit audit event with allowed=false — got: {:?}",
            audit_events
                .iter()
                .map(|e| (&e.action, e.allowed, &e.reason))
                .collect::<Vec<_>>()
        );

        cleanup_workspace(&ws);
    }
}
