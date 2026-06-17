# Permission Policy Matrix

The permission policy defines how tool invocations are handled across different tool sources and execution modes.

## Policy Matrix

| Source \ Mode | Chat | Plan | Bypass |
|---|---|---|---|
| **Native Low** | Allow | Allow | Allow |
| **Native Medium** | Allow | Allow | Allow |
| **Native High** | Confirm | Confirm | Allow |
| **Native Critical** | Deny | Deny | Allow |
| **WASM Low** | Allow | Allow | Allow |
| **WASM Medium** | Deny | Deny | Allow |
| **WASM High** | Deny | Deny | Allow |
| **WASM Critical** | Deny | Deny | Allow |
| **MCP Low** | Allow | Allow | Allow |
| **MCP Medium** | Allow | Confirm | Allow |
| **MCP High** | Confirm | Confirm | Allow |
| **MCP Critical** | Deny | Deny | Allow |

## Definitions

### Tool Sources

- **Native** — Built-in Rust tools (shell, read_file, write_file, etc.)
- **WASM** — WASM plugin tools (untrusted third-party code)
- **MCP** — MCP server tools (external protocol servers)

### Execution Modes

- **Chat** — Normal interactive mode. User directly controls the conversation.
- **Plan** — Planning mode. Higher scrutiny for potentially dangerous operations.
- **Bypass** — Admin/test context. All operations allowed without confirmation.

### Risk Levels

- **Low** — Read-only, non-destructive operations (ls, cat, grep, status)
- **Medium** — Potentially impactful but common operations (git push, clone)
- **High** — Destructive or irreversible operations (rm -rf, git reset --hard)
- **Critical** — System-level or dangerous operations (sudo, dd, fork-bomb)

### Policy Decisions

- **Allow** — Execute immediately without user interaction
- **Confirm** — Prompt user for confirmation before execution
- **Deny** — Block execution entirely

## Implementation

The policy is implemented in `runtime/tools/src/policy.rs` via `evaluate_policy(source, mode, risk)`. Shell Gate (`runtime/tools/src/shell_gate/`) classifies shell commands into risk levels. WASM host imports enforce deny-by-default for medium+ risk.

## Key Design Decisions

1. **WASM plugins are more restrictive** — Third-party WASM code cannot request confirmation (no interactive channel in WASM context), so medium+ risk operations are denied outright.

2. **Native High requires confirmation** — Destructive commands like `rm -rf` in the workspace require user approval in both Chat and Plan modes.

3. **Bypass mode is unrestricted** — Used for testing and admin operations where the caller has full authority.

4. **MCP Medium requires confirmation in Plan mode** — External MCP tools get intermediate trust: medium-risk operations are allowed in Chat but require confirmation in Plan mode.
