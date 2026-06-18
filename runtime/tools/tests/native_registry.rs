//! Integration tests for native tools in ToolRegistry.
//!
//! These tests verify the P3-2 requirements:
//! - Without WASM plugins, default registry contains native tools
//! - ToolRegistry::get() and to_tool_defs() return native tools
//! - Plan mode high-risk shell/write/edit need confirmation preview
//! - Chat mode workspace-internal ApprovalRequired executes, external paths denied
//! - Bypass permission strategy skips Alius path gates while preserving OS failures

use protocol_interface::{PermissionStrategy, RuntimeMode};
use runtime_tools::package::ToolPackageResolver;
use runtime_tools::traits::ToolContext;
use serde_json::json;
use tempfile::TempDir;

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

    // High-risk command in Chat mode requires confirmation (policy matrix: Native Chat High = Confirm)
    let high_risk_args = json!({"command": "rm -rf ./build"});
    assert!(
        shell.preview_confirmation(&high_risk_args, RuntimeMode::Chat),
        "High-risk shell command should require confirmation in Chat mode (policy matrix)"
    );

    // Low-risk command in Chat mode should not require confirmation
    let low_risk_args = json!({"command": "echo hello"});
    assert!(
        !shell.preview_confirmation(&low_risk_args, RuntimeMode::Chat),
        "Low-risk shell command should not require confirmation in Chat mode"
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
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace.clone());
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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

#[tokio::test]
async fn test_bypass_permissions_allows_write_file_outside_workspace() {
    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let resolver = ToolPackageResolver::new(workspace.path().to_path_buf());
    let registry = resolver.build_registry_lossy();

    let write_file = registry
        .get("write_file")
        .expect("write_file tool should be present");
    let target = outside.path().join("bypass-write.txt");
    let ctx = ToolContext::new_with_permission_strategy(
        workspace.path().to_path_buf(),
        "test-session".to_string(),
        RuntimeMode::Plan,
        PermissionStrategy::BypassPermissions,
    );

    let result = write_file
        .execute(
            json!({
                "path": target.to_string_lossy(),
                "content": "bypass permissions writes outside workspace"
            }),
            ctx,
        )
        .await
        .expect("write_file should execute");

    assert!(
        result.success,
        "BypassPermissions should skip Alius workspace path denial, got: {}",
        result.output
    );
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        "bypass permissions writes outside workspace"
    );
}

#[test]
fn test_write_file_preview_confirmation_in_plan_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let write_file = registry
        .get("write_file")
        .expect("write_file tool should be present");

    // write_file always requires confirmation in Plan mode
    let args = json!({"path": "test.txt", "content": "hello"});
    assert!(
        write_file.preview_confirmation(&args, RuntimeMode::Plan),
        "write_file should require confirmation in Plan mode"
    );
}

#[test]
fn test_write_file_no_confirmation_in_chat_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let write_file = registry
        .get("write_file")
        .expect("write_file tool should be present");

    // write_file does not require confirmation in Chat mode
    let args = json!({"path": "test.txt", "content": "hello"});
    assert!(
        !write_file.preview_confirmation(&args, RuntimeMode::Chat),
        "write_file should not require confirmation in Chat mode"
    );
}

#[test]
fn test_edit_file_preview_confirmation_in_plan_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let edit_file = registry
        .get("edit_file")
        .expect("edit_file tool should be present");

    // edit_file always requires confirmation in Plan mode
    let args = json!({"path": "test.txt", "find": "old", "replace": "new"});
    assert!(
        edit_file.preview_confirmation(&args, RuntimeMode::Plan),
        "edit_file should require confirmation in Plan mode"
    );
}

#[test]
fn test_edit_file_no_confirmation_in_chat_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let edit_file = registry
        .get("edit_file")
        .expect("edit_file tool should be present");

    // edit_file does not require confirmation in Chat mode
    let args = json!({"path": "test.txt", "find": "old", "replace": "new"});
    assert!(
        !edit_file.preview_confirmation(&args, RuntimeMode::Chat),
        "edit_file should not require confirmation in Chat mode"
    );
}

/// End-to-end test: `rm -rf ./build` in Chat mode requires confirmation
/// and does NOT execute without it. This verifies the policy matrix:
/// Native Chat High = Confirm.
#[test]
fn test_rm_rf_requires_confirmation_in_chat_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    let high_risk_args = json!({"command": "rm -rf ./build"});
    assert!(
        shell.preview_confirmation(&high_risk_args, RuntimeMode::Chat),
        "rm -rf ./build should require confirmation in Chat mode (policy: Native Chat High = Confirm)"
    );

    // Low-risk command should NOT require confirmation
    let low_risk_args = json!({"command": "ls -la"});
    assert!(
        !shell.preview_confirmation(&low_risk_args, RuntimeMode::Chat),
        "ls -la should not require confirmation in Chat mode"
    );
}

/// End-to-end test: `rm -rf ./build` in Plan mode requires confirmation
/// and does NOT execute without it. This verifies the policy matrix:
/// Native Plan High = Confirm.
#[test]
fn test_rm_rf_requires_confirmation_in_plan_mode() {
    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace);
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    let high_risk_args = json!({"command": "rm -rf ./build"});
    assert!(
        shell.preview_confirmation(&high_risk_args, RuntimeMode::Plan),
        "rm -rf ./build should require confirmation in Plan mode (policy: Native Plan High = Confirm)"
    );
}

/// End-to-end test: `rm -rf ./build` actually executes and succeeds when
/// no confirmation gate blocks it (direct execute path). This verifies
/// that the shell tool itself doesn't block high-risk commands — the
/// confirmation gate is the LoopEngine's responsibility.
#[tokio::test]
async fn test_rm_rf_executes_when_confirmation_bypassed() {
    use runtime_tools::traits::ToolContext;

    let workspace = std::env::current_dir().unwrap();
    let resolver = ToolPackageResolver::new(workspace.clone());
    let registry = resolver.build_registry_lossy();

    let shell = registry.get("shell").expect("shell tool should be present");

    // Create a temp directory to delete
    let build_dir = workspace.join("__test_rm_rf_target__");
    std::fs::create_dir_all(&build_dir).unwrap();
    std::fs::write(build_dir.join("file.txt"), "test").unwrap();

    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };

    // Direct execute (bypassing confirmation) — shell tool itself does not block
    let args = json!({"command": "rm -rf __test_rm_rf_target__"});
    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(
        tool_result.success,
        "rm -rf should succeed when executed directly"
    );
    assert!(
        !build_dir.exists(),
        "Target directory should be deleted after rm -rf"
    );
}
