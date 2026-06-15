# P3-2 实施总结：Native Tools 进入默认 ToolRegistry

**实施日期：** 2026-06-16  
**分支：** fix/tools-native-registry-confirmation  
**状态：** ✅ 完成并通过质量门

---

## 执行概览

### 目标

让 native tools 真正进入默认 ToolRegistry，并补齐注册与确认链路验收。现在文档要求 native tools 自动注册，但 ToolPackageResolver::build_registry() 仍只注册 WASM 工具，这是下一个必须修的功能缺口。

### ✅ 已完成任务

#### 1. Native Tools 注册到默认 Registry

**问题：**
- `ToolPackageResolver::build_registry()` 只加载 WASM 工具
- 无 WASM 插件时，registry 为空
- Native tools 不会自动注册

**解决方案：**
```rust
// runtime/tools/src/package.rs
pub fn build_registry(&self) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    // Always register native tools first
    crate::native::register_native_tools(&mut registry);
    for package in self.list_installed_packages()? {
        // ... load WASM tools
    }
    Ok(registry)
}

pub fn build_registry_lossy(&self) -> ToolRegistry {
    match self.build_registry() {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("[warn] Failed to load Rust WASM tools: {err}");
            // Still register native tools even if WASM loading fails
            let mut registry = ToolRegistry::new();
            crate::native::register_native_tools(&mut registry);
            registry
        }
    }
}
```

**关键变更：**
- `build_registry()` 始终先注册 native tools
- `build_registry_lossy()` 即使 WASM 加载失败，也注册 native tools
- 确保无 WASM 插件时，默认 registry 包含所有 native tools

#### 2. Registry 查询 API 验证

**验证内容：**
- `ToolRegistry::get()` 能返回 native tools
- `ToolRegistry::to_tool_defs()` 能返回 native tools
- Native tools 有有效的 input schema

**测试覆盖：**
```rust
// runtime/tools/src/registry.rs
#[test]
fn test_native_tools_registered() { ... }

#[test]
fn test_get_native_tools() { ... }

#[test]
fn test_to_tool_defs_includes_native() { ... }
```

#### 3. Confirmation Preview 链路验证

**验证内容：**
- Plan 模式下高风险 shell/write/edit 命令需要确认
- Chat/Bypass 模式下不需要确认

**测试覆盖：**
```rust
// runtime/tools/tests/native_registry.rs
#[test]
fn test_shell_preview_confirmation_in_plan_mode() { ... }

#[test]
fn test_shell_preview_confirmation_in_chat_mode() { ... }
```

#### 4. Chat/Bypass 模式行为验证

**验证内容：**
- Workspace 内 ApprovalRequired 命令可执行
- Workspace 外路径仍硬拒绝

**测试覆盖：**
```rust
// runtime/tools/tests/native_registry.rs
#[tokio::test]
async fn test_chat_mode_workspace_internal_executes() { ... }

#[tokio::test]
async fn test_chat_mode_external_path_denied() { ... }
```

---

## 质量门验收

```bash
✓ cargo fmt --all -- --check
✓ cargo check --workspace --all-targets --all-features
✓ cargo clippy --workspace --all-targets --all-features -- -D warnings
✓ cargo test -p runtime-tools --test native_registry -- --test-threads=1 (8 tests)
✓ cargo test -p runtime-tools --test shell_gate_integration -- --test-threads=1 (25 tests)
✓ cargo test --workspace -- --test-threads=1 (480+ tests)
```

**测试统计：**
- registry.rs 单元测试：3 个
- native_registry.rs 集成测试：8 个
- 总计 P3-2 相关测试：11 个

---

## 修改文件

1. **runtime/tools/src/package.rs**
   - `build_registry()` 始终先注册 native tools
   - `build_registry_lossy()` 即使 WASM 失败也注册 native tools

2. **runtime/tools/src/registry.rs**
   - 新增 3 个单元测试验证 native tools 注册和查询

3. **runtime/tools/tests/native_registry.rs**（新文件）
   - 新增 8 个集成测试
   - 验证默认 registry 包含 native tools
   - 验证 get() 和 to_tool_defs() 返回 native tools
   - 验证 Plan 模式 confirmation preview
   - 验证 Chat/Bypass 模式 workspace 内可执行、外路径拒绝

4. **.alius/workspace/docs/modules/tools-and-shell-gate.md**
   - 文档已说明 native tools 是默认注册的

5. **.alius/workspace/HISTORY.md**
   - 添加 P3-2 HISTORY 记录

---

## 功能验收结果

✅ **无 WASM 插件时，默认 registry 仍包含 native tools**
- shell, read_file, write_file, list_dir, edit_file 全部注册

✅ **ToolRegistry::get() 和 to_tool_defs() 能返回 native tools**
- get("shell") 返回 Shell 工具
- to_tool_defs() 包含所有 native tools

✅ **Plan 模式下高风险 shell/write/edit 会走 confirmation preview**
- rm -rf ./build 在 Plan 模式需要确认
- ls -la 在 Plan 模式不需要确认

✅ **Chat/Bypass 模式下 workspace 内 ApprovalRequired 可执行**
- echo hello 在 Chat 模式可执行
- workspace 外路径 cat /etc/passwd 被拒绝

✅ **workspace 外路径仍硬拒绝**
- 无论 RuntimeMode 如何，外部路径命令都被拒绝

---

## 设计依据

- **SPEC.md** - Native tools 注册要求
- **tools-and-shell-gate.md** - 工具实现规则和注册机制
- **P3-2 任务要求** - Native tools 进入默认 ToolRegistry

---

## 后续工作

P3-2 完成后，下一步是 P3-3（待定）。

当前实现已满足所有功能验收要求：
- ✅ Native tools 自动注册到默认 registry
- ✅ Registry 查询 API 正确返回 native tools
- ✅ Confirmation preview 链路完整
- ✅ Chat/Bypass 模式行为正确
- ✅ 测试覆盖充分
- ✅ 质量门全部通过

P3-2 确保了 native tools 在任何情况下都能被发现和使用。
