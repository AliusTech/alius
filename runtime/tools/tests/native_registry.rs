//! Integration tests for native tools in ToolRegistry.
//!
//! These tests verify the P3-2 requirements:
//! - Without WASM plugins, default registry contains native tools
//! - ToolRegistry::get() and to_tool_defs() return native tools
//! - Plan mode high-risk shell/write/edit need confirmation preview
//! - Chat/Bypass mode workspace-internal ApprovalRequired executes, external paths denied

use protocol_interface::RuntimeMode;
use runtime_tools::package::ToolPackageResolver;
use serde_json::json;

#[test]
fn test_default_registry_contains_native_tools() {
    // Create a resolver with a workspace that has no WASM plugins
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);

    // Build registry (WASM loading might fail, but native should still work)
    let registry = resolver.build_registry_lossy();

    // Verify all native tools are present
    assert!(registry.has("shell"));
    assert!(registry.has("read_file"));
    assert!(registry.has("write_file"));
    assert!(registry.has("list_dir"));
    assert!(registry.has("edit_file"));
}

#[test]
fn test_get_returns_native_tools() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    // Test get() for each native tool
    let shell = registry.get("shell").expect("shell tool should be present");
    assert_eq!(shell.name(), "shell");
    assert!(!shell.description().is_empty());

    let read_file = registry
        .get("read_file")
        .expect("read_file tool should be present");
    assert_eq!(read_file.name(), "read_file");
}

#[test]
fn test_to_tool_defs_includes_native_tools() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let tool_defs = registry.to_tool_defs();
    let names: Vec<String> = tool_defs.iter().map(|t| t.name.clone()).collect();

    assert!(
        names.contains(&"shell".to_string()),
        "shell should be in tool defs"
    );
    assert!(
        names.contains(&"read_file".to_string()),
        "read_file should be in tool defs"
    );
    assert!(
        names.contains(&"write_file".to_string()),
        "write_file should be in tool defs"
    );
    assert!(
        names.contains(&"list_dir".to_string()),
        "list_dir should be in tool defs"
    );
    assert!(
        names.contains(&"edit_file".to_string()),
        "edit_file should be in tool defs"
    );
}

#[test]
fn test_native_tools_have_valid_schemas() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    for name in &["shell", "read_file", "write_file", "list_dir", "edit_file"] {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("{} should be present", name));
        let schema = tool.input_schema();
        assert!(schema.is_object(), "{} should have object schema", name);
    }
}

#[test]
fn test_shell_preview_confirmation_in_plan_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    // High-risk command in Plan mode should require confirmation
    let high_risk_args = json!({"command": "rm -rf ./build"});
    assert!(
        shell.preview_confirmation(&high_risk_args, RuntimeMode::Plan),
        "High-risk shell command should require confirmation in Plan mode"
    );

    // Low-risk command in Plan mode should not require confirmation
    let low_risk_args = json!({"command": "ls -la"});
    assert!(
        !shell.preview_confirmation(&low_risk_args, RuntimeMode::Plan),
        "Low-risk shell command should not require confirmation in Plan mode"
    );
}

#[test]
fn test_shell_preview_confirmation_in_chat_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    // High-risk command in Chat mode should not require confirmation (runs directly)
    let high_risk_args = json!({"command": "rm -rf ./build"});
    assert!(
        !shell.preview_confirmation(&high_risk_args, RuntimeMode::Chat),
        "Shell command should not require confirmation in Chat mode"
    );
}

#[tokio::test]
async fn test_chat_mode_workspace_internal_executes() {
    use runtime_tools::traits::ToolContext;

    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace.clone());
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
    };

    // Low-risk workspace-internal command should execute
    let args = json!({"command": "echo hello"});
    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success, "Low-risk command should succeed");
    assert!(
        tool_result.output.contains("hello"),
        "Output should contain 'hello'"
    );
}

#[tokio::test]
async fn test_chat_mode_external_path_denied() {
    use runtime_tools::traits::ToolContext;

    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace.clone());
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
    };

    // External path command should be denied even in Chat mode
    let args = json!({"command": "cat /etc/passwd"});
    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(
        !tool_result.success,
        "External path command should be denied"
    );
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Should contain denial message"
    );
}
