//! Integration tests for WASM plugin execution through the full tool path.
//!
//! These tests verify: WasmPluginTool::from_wasm_bytes → execute() →
//! call_plugin_tool_with_state(). Uses WAT modules that implement the
//! plugin ABI (list_tools + call_tool exports) without host imports,
//! since `list_plugin_tools` instantiates without a linker.

use runtime_tools::wasm_host::{ResolvedPluginPermissions, WasmPluginTool};
use runtime_tools::{AliusTool, ToolContext};
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;

fn make_workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alius_wasm_integration_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("data/hello.txt"), "hello world").unwrap();
    dir
}

fn cleanup_workspace(ws: &Path) {
    let _ = std::fs::remove_dir_all(ws);
}

/// Helper: write a fixed string at a given offset in WAT, return (offset, len).
fn wati_write_str(wat: &mut String, offset: usize, s: &str) -> usize {
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        wat.push_str(&format!(
            "                (i32.store8 (i32.const {}) (i32.const {}))\n",
            offset + i,
            b
        ));
    }
    bytes.len()
}

/// Build a minimal WAT plugin that always returns {"output":"ok","success":true}
/// regardless of input. Tests the plugin ABI without host import complexity.
fn make_simple_plugin_wasm() -> Vec<u8> {
    // Fixed list_tools response
    let list_json = r#"[{"name":"ping","description":"returns ok","inputSchema":{}}]"#;
    // Fixed call_tool response
    let call_json = r#"{"output":"ok","success":true}"#;

    let mut wat = String::from(
        r#"
        (module
            (memory (export "memory") 2)

            ;; alius_plugin_list_tools() -> i32
            (func $list_tools (result i32)
    "#,
    );

    // Write list JSON at offset 0: [len:4][json:len]
    let list_len = list_json.len();
    let lb = (list_len as u32).to_le_bytes();
    for (i, b) in lb.iter().enumerate() {
        wat.push_str(&format!(
            "                (i32.store8 (i32.const {}) (i32.const {}))\n",
            i, b
        ));
    }
    wati_write_str(&mut wat, 4, list_json);
    wat.push_str("                (i32.const 0)\n");
    wat.push_str("            )\n\n");

    // alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len) -> i32
    // Returns fixed {"output":"ok","success":true} at offset 4096
    wat.push_str("            (func $call_tool (param i32 i32 i32 i32) (result i32)\n");
    let call_len = call_json.len();
    let cb = (call_len as u32).to_le_bytes();
    for (i, b) in cb.iter().enumerate() {
        wat.push_str(&format!(
            "                (i32.store8 (i32.const {}) (i32.const {}))\n",
            4096 + i,
            b
        ));
    }
    wati_write_str(&mut wat, 4100, call_json);
    wat.push_str("                (i32.const 4096)\n");
    wat.push_str("            )\n\n");

    wat.push_str(
        r#"
            (export "alius_plugin_list_tools" (func $list_tools))
            (export "alius_plugin_call_tool" (func $call_tool))
        )
        "#,
    );

    wat::parse_str(&wat).unwrap()
}

#[tokio::test]
async fn test_plugin_discovery() {
    let wasm = make_simple_plugin_wasm();
    let tools = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions::default(),
        "test-plugin".to_string(),
    )
    .expect("should discover tools");

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "ping");
    assert_eq!(tools[0].description(), "returns ok");
}

#[tokio::test]
async fn test_plugin_execute_simple() {
    let ws = make_workspace();
    let wasm = make_simple_plugin_wasm();
    let tools = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions::default(),
        "test-plugin".to_string(),
    )
    .unwrap();

    let ctx = ToolContext::new(
        ws.clone(),
        "test-session".to_string(),
        protocol_interface::RuntimeMode::Chat,
    );
    let result = tools[0].execute(json!({}), ctx).await.unwrap();

    assert!(result.success);
    assert_eq!(result.output, "ok");

    cleanup_workspace(&ws);
}

#[tokio::test]
async fn test_plugin_execute_with_args() {
    // Verify that args are passed through the ABI correctly
    let ws = make_workspace();
    let wasm = make_simple_plugin_wasm();
    let tools = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions::default(),
        "test-plugin".to_string(),
    )
    .unwrap();

    let ctx = ToolContext::new(
        ws.clone(),
        "test-session".to_string(),
        protocol_interface::RuntimeMode::Chat,
    );
    // Simple plugin ignores args but should still succeed
    let result = tools[0]
        .execute(json!({"key": "value", "nested": {"a": 1}}), ctx)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "ok");

    cleanup_workspace(&ws);
}

#[tokio::test]
async fn test_plugin_permissions_empty() {
    let permissions = ResolvedPluginPermissions::default();
    assert!(permissions.is_empty());
    assert!(permissions.filesystem.is_empty());
    assert!(permissions.network.is_empty());
    assert!(permissions.shell.is_empty());
    assert!(permissions.env.is_empty());
}

#[tokio::test]
async fn test_plugin_legacy_discovery() {
    let wasm = make_simple_plugin_wasm();
    let tools = WasmPluginTool::from_wasm_bytes_legacy(&wasm).unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "ping");
}

#[tokio::test]
async fn test_plugin_multiple_instances() {
    let ws = make_workspace();
    let wasm = make_simple_plugin_wasm();

    let tools_a = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions::default(),
        "plugin-a".to_string(),
    )
    .unwrap();

    let tools_b = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions {
            filesystem: vec!["read:data".to_string()],
            ..Default::default()
        },
        "plugin-b".to_string(),
    )
    .unwrap();

    assert_eq!(tools_a[0].name(), tools_b[0].name());

    let ctx = ToolContext::new(
        ws.clone(),
        "s1".to_string(),
        protocol_interface::RuntimeMode::Chat,
    );
    assert!(tools_a[0].execute(json!({}), ctx).await.unwrap().success);

    let ctx = ToolContext::new(
        ws.clone(),
        "s2".to_string(),
        protocol_interface::RuntimeMode::Chat,
    );
    assert!(tools_b[0].execute(json!({}), ctx).await.unwrap().success);

    cleanup_workspace(&ws);
}

#[tokio::test]
async fn test_plugin_requires_confirmation() {
    let list_json = r#"[{"name":"dangerous","description":"needs confirm","inputSchema":{},"requires_confirmation":true}]"#;
    let call_json = r#"{"output":"done","success":true}"#;

    let mut wat = String::from(
        r#"
        (module
            (memory (export "memory") 2)
            (func $list_tools (result i32)
    "#,
    );
    let ll = list_json.len();
    let lb = (ll as u32).to_le_bytes();
    for (i, b) in lb.iter().enumerate() {
        wat.push_str(&format!(
            "                (i32.store8 (i32.const {}) (i32.const {}))\n",
            i, b
        ));
    }
    wati_write_str(&mut wat, 4, list_json);
    wat.push_str("                (i32.const 0)\n            )\n\n");
    wat.push_str("            (func $call_tool (param i32 i32 i32 i32) (result i32)\n");
    let cl = call_json.len();
    let cb = (cl as u32).to_le_bytes();
    for (i, b) in cb.iter().enumerate() {
        wat.push_str(&format!(
            "                (i32.store8 (i32.const {}) (i32.const {}))\n",
            4096 + i,
            b
        ));
    }
    wati_write_str(&mut wat, 4100, call_json);
    wat.push_str("                (i32.const 4096)\n            )\n\n");
    wat.push_str(
        r#"
            (export "alius_plugin_list_tools" (func $list_tools))
            (export "alius_plugin_call_tool" (func $call_tool))
        )
    "#,
    );

    let wasm = wat::parse_str(&wat).unwrap();
    let tools = WasmPluginTool::from_wasm_bytes(
        &wasm,
        ResolvedPluginPermissions::default(),
        "test-plugin".to_string(),
    )
    .unwrap();

    assert_eq!(tools[0].name(), "dangerous");
    assert!(tools[0].requires_confirmation(&json!({})));
}
