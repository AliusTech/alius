# P3-1 实施总结：Shell Gate 参数级影响范围与 workspace 边界加固

**实施日期：** 2026-06-16  
**分支：** fix/tools-shell-scope-args  
**状态：** ✅ 完成并通过质量门

---

## 执行概览

### 目标

确保 `shell` 工具执行前，Shell Gate 必须基于真实命令参数、重定向目标、cwd 和 workspace root 做范围判断，不能出现"只检查 command、不检查 args"或 workspace 外路径在 Chat/Bypass 下静默执行的问题。

### ✅ 已完成任务

#### 1. 参数级范围分析增强

**问题：** 
- `scope_args()` 已经正确实现了 fallback（args 为空时解析 command）
- 但重定向符号（`>`, `2>`）在 args 中不存在，导致重定向目标未被检测

**解决方案：**
```rust
// runtime/tools/src/shell_gate/scope.rs
pub fn analyze_scope(request: &ShellCommandRequest) -> ScopeAnalysis {
    // ... 从 args 提取路径候选
    
    // 新增：从原始 command 中提取重定向目标
    for candidate in extract_redirections_from_command(&request.command) {
        if let Some(path) = resolve_candidate_path(&candidate, &request.cwd) {
            if !is_inside_workspace(&path, &request.workspace_root) {
                external_paths.push(path);
            }
        }
    }
    // ...
}

fn extract_redirections_from_command(command: &str) -> Vec<String> {
    // 解析 "echo test > /tmp/out" 中的 /tmp/out
    // 解析 "command 2>/tmp/err" 中的 /tmp/err
}
```

**覆盖场景：**
- ✅ `/etc/passwd` - 绝对路径
- ✅ `../outside` - 相对路径逃逸
- ✅ `--output=/tmp/out` - 长选项值
- ✅ `> /tmp/out` - stdout 重定向
- ✅ `2>/tmp/err` - stderr 重定向

#### 2. cwd 边界验证

**问题：**
- `resolve_cwd()` 仅拼接路径，未验证结果在 workspace 内
- 绝对路径和 `../` 逃逸可以绕过边界

**解决方案：**
```rust
// runtime/tools/src/native/shell.rs
fn resolve_cwd(cwd: Option<&str>, workspace: &Path) -> Result<PathBuf, AliusError> {
    let candidate = match cwd {
        Some(c) if !c.is_empty() => {
            let path = Path::new(c);
            // 拒绝绝对路径
            if path.is_absolute() {
                return Err(AliusError::Agent(
                    "cwd must be relative to workspace".to_string(),
                ));
            }
            workspace.join(c)
        }
        _ => workspace.to_path_buf(),
    };

    // Canonicalize 解析 .. 和 symlinks
    let canonical = candidate.canonicalize()
        .map_err(|e| AliusError::Agent(format!("invalid cwd: {e}")))?;
    
    let canonical_workspace = workspace.canonicalize()
        .map_err(|e| AliusError::Agent(format!("failed to canonicalize workspace: {e}")))?;

    // 验证解析后的 cwd 在 workspace 内
    if !canonical.starts_with(&canonical_workspace) {
        return Err(AliusError::Agent("cwd must be inside workspace".to_string()));
    }

    Ok(canonical)
}
```

**验证：**
- ✅ 拒绝绝对路径 `/tmp`
- ✅ 拒绝 `../` 逃逸出 workspace
- ✅ 检测 symlink 逃逸
- ✅ 默认值使用 workspace root

#### 3. 测试覆盖

**单元测试（scope.rs）：**
- 16 个测试用例覆盖所有路径检测场景
- `test_etc_passwd_is_external`
- `test_parent_escape_outside_is_external`
- `test_output_flag_with_external_path`
- `test_redirection_operator_with_tmp`
- `test_stderr_redirection_to_tmp`
- `test_cwd_absolute_path_outside_workspace`
- `test_cwd_parent_escape_outside_workspace`
- `test_args_override_empty_args_fallback`

**单元测试（shell.rs）：**
- 5 个测试用例验证 cwd 边界检查
- `test_resolve_cwd_relative_path_ok`
- `test_resolve_cwd_absolute_path_rejected`
- `test_resolve_cwd_parent_escape_rejected`
- `test_resolve_cwd_empty_defaults_to_workspace`
- `test_resolve_cwd_nonexistent_path_rejected`

**集成测试（shell_gate_integration.rs）：**
- 10 个测试用例验证端到端功能
- 覆盖所有功能验收场景
- 验证 args 使用和 fallback 逻辑
- 验证多个外部路径检测

### 功能验收结果

✅ **args 携带真实参数**
- `ShellCommandRequest.args` 通过 `command_args(&command)` 填充
- 不能传空数组绕过 scope 分析

✅ **args 使用和 fallback**
- 显式 args 存在时使用显式 args
- args 为空时 fallback 解析 raw command
- `scope_args()` 正确实现

✅ **外部路径识别**
- `/etc/passwd` ✓
- `../outside` ✓
- `--output=/tmp/out` ✓
- `> /tmp/out` ✓
- `2>/tmp/err` ✓

✅ **cwd workspace 限制**
- 绝对路径拒绝 ✓
- `../` 逃逸拒绝 ✓
- symlink 逃逸检测 ✓

✅ **Chat/Bypass 模式不绕过 Shell Gate**
- shell.rs 第 89-95 行构造 `ShellCommandRequest`
- 第 96-98 行调用 `authorize()` 检查
- `ShellGateDecision::Deny` 直接返回错误
- 不依赖 RuntimeMode 做门控判断

✅ **Plan 模式确认流程**
- 高风险命令返回 `ApprovalRequired`
- workspace 外路径通过 scope 分析硬拒绝

---

## 质量门验收

```bash
✓ cargo fmt --all -- --check
✓ cargo check --workspace --all-targets --all-features
✓ cargo clippy --workspace --all-targets --all-features -- -D warnings
✓ cargo test -p runtime-tools shell_gate -- --test-threads=1 (39 tests)
✓ cargo test -p runtime-tools --test shell_gate_integration -- --test-threads=1 (10 tests)
✓ cargo test --workspace -- --test-threads=1 (434 tests passed)
```

**测试统计：**
- runtime-tools: 55 个测试（新增 21 个）
- 集成测试: 10 个测试（新增）
- 完整测试套件: 434 个测试全部通过

---

## 修改文件

1. **runtime/tools/src/shell_gate/scope.rs**
   - 新增 `extract_redirections_from_command()` 函数
   - 修改 `analyze_scope()` 同时检查 args 和原始 command
   - 新增 8 个测试用例

2. **runtime/tools/src/native/shell.rs**
   - 修改 `resolve_cwd()` 返回 `Result<PathBuf, AliusError>`
   - 添加绝对路径检查和 workspace 边界验证
   - 更新调用点处理错误
   - 新增 5 个测试用例

3. **runtime/tools/tests/shell_gate_integration.rs**（新文件）
   - 10 个端到端集成测试
   - 验证所有功能验收要求

---

## 影响范围

**无 API breaking changes**

- `ShellCommandRequest` 结构未变
- `analyze_scope()` 签名未变
- `resolve_cwd()` 从返回 `PathBuf` 改为返回 `Result<PathBuf, AliusError>`
  - 仅在 shell.rs 内部使用
  - 调用点已更新处理错误

**行为变更：**
- cwd 使用绝对路径现在返回错误（之前允许）
- cwd 逃逸出 workspace 现在返回错误（之前允许）
- 重定向目标现在被检测为外部路径（之前未检测）

这些都是**安全性增强**，符合设计意图。

---

## 设计依据

- **SPEC.md** - Shell Gate 职责和边界定义
- **tools-and-shell-gate.md** - 工具实现规则和 workspace 边界
- **P3-1 任务要求** - 参数级影响范围与边界加固

---

## 后续工作

P3-1 完成后，下一步是 P3-2（待定）。

当前实现已满足所有功能验收要求：
- ✅ 参数级范围分析
- ✅ workspace 边界加固
- ✅ 外部路径检测完整
- ✅ cwd 验证严格
- ✅ 测试覆盖充分
- ✅ 质量门全部通过

P3-1 为 Shell Gate 提供了更严格的安全边界和更完整的路径检测能力。
