# Protocol Interface

The Protocol Interface is the boundary between product entrypoints and Core Runtime.

Current code lives in:

- `protocol/src/core.rs`
- `protocol/src/interface.rs`
- `protocol/src/error.rs`
- `protocol/src/types.rs`

## Main Contract Types

| Type | Purpose |
| --- | --- |
| `ProtocolEnvelope<T>` | Shared wrapper for requests, commands, and events. |
| `Origin` | Product or adapter identity. |
| `CapabilityScope` | Capability ceiling supplied by the origin. |
| `Capability` | Individual capability flags. |
| `CoreRequest` | Starts or inspects work. |
| `CoreCommand` | Controls an existing run. |
| `CoreEvent` | Reports run progress and results. |
| `ProtocolError` | Boundary error model. |
| `CoreRuntimeApi` | Trait implemented by Core Runtime. |

## Envelope

`ProtocolEnvelope<T>` carries:

- `protocol_version`
- `origin`
- `capability_scope`
- `workspace_root`
- `session_ref`
- `run_ref`
- `trace_id`
- `payload`

The protocol version is validated before delegation.

## Origin

Current origins include:

- `LocalCli`
- `LocalTui`
- `EmbeddedSdk`
- `IdeExtension`
- `Desktop`
- `RemoteA2A`
- `PluginRpc`
- `JsonRpc`
- `Test`

Origin should be treated as caller identity for policy and capability decisions, not as a UI label.

## Capability Scope

`CapabilityScope` is a ceiling supplied by the caller. It is not final authorization.

Current convenience scopes include:

- `local_cli`
- `local_tui`
- `embedded_sdk`
- `remote_a2a`

Capabilities include workspace, model, tools, shell, MCP, memory, config, and remote A2A access.

## Requests

`CoreRequestKind` includes:

- `InitProject`
- `RunLoop`
- `StartTurn`
- `OpenSession`
- `InspectSession`
- `ListSessions`
- `ToolQuery`
- `CloseSession`
- `ClearConversation`
- `ConfigRead`
- `ConfigValidate`
- `ConfigUpdate`
- `ModelList`
- `MemorySave`
- `MemoryList`
- `MemoryClear`
- `ReviewStart`
- `ReviewToggle`
- `ConfirmToggle`
- `HealthCheck`
- `SessionCommand`

`CoreRequest::run_loop` validates non-empty input and carries a `RunLoopInput`.

## Commands

`CoreCommandKind` includes:

- `Cancel`
- `Approve`
- `Deny`
- `Continue`
- `Pause`
- `ApprovePlan`
- `RevisePlan`
- `ExecuteSelected`
- `ApproveReview`
- `RequestRevision`
- `SwitchModel`
- `SwitchMode`

Commands target an existing `RunRef`.

## Events

Core events carry event id, trace id, optional session and turn refs, run ref, sequence, kind, payload, and creation time.

Event kinds include run lifecycle, model deltas, tool calls, convergence, approvals, user input, errors, and final results.

## Gateway

`ProtocolInterface<R>` wraps a `CoreRuntimeApi` implementation.

Responsibilities:

- Validate request and command envelopes.
- Store run protocol context.
- Start execution.
- Send commands.
- Subscribe to events.
- Wrap events back into protocol envelopes.
- Apply capability checks for config, model, session, memory, tool, review, and log operations.

