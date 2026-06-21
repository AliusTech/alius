# Agent Team Module

Agent Team is the planned multi-agent coordination surface for Alius. It connects
local Agent CLI instances to an Agent Team Backend through a long-lived outbound
connection. The backend maintains agent presence, work status, task leases, and
team event streams.

This module is a design contract. The current product still treats live Agent
Team and A2A traffic as not connected by default until the runtime connector,
backend protocol, and tests are implemented.

## Terminology

Use these names consistently:

| Term | Meaning |
| --- | --- |
| Agent | A local collaborative agent identity with a Soul, role, capabilities, and workspace context. |
| Agent CLI | The Rust CLI process that runs local runtime work and connects the Agent to the team backend. |
| Agent Team Backend | The server-side coordination API. The current backend plan uses FastAPI. |
| Agent Connection | One long-lived WebSocket session between an Agent CLI and the backend. Connections can reconnect and expire. |
| Agent Presence | The connection-level state: connecting, online, syncing, degraded, reconnecting, or offline. |
| Agent Work Status | The execution-level state: idle, planning, running, streaming, waiting for approval, running tool, reviewing, blocked, completed, failed, or cancelled. |
| Agent Task Lease | A backend-issued lease proving that one Agent CLI currently owns a task execution. |

The product subject is the Agent, and the local carrier is the Agent CLI.
Agent Team documentation must use Agent terminology consistently.

## Transport Choice

The first implementation should use WebSocket:

```text
Agent CLI --wss--> Agent Team Backend
```

The Agent CLI initiates the outbound connection. It must not expose an inbound
port for the backend to call. The backend should expose a WebSocket route on the
same HTTPS service as the REST API, for example:

```text
GET /api/agent/ws
```

Production traffic must use `wss://` over port 443. Development may use
`ws://localhost:<port>/api/agent/ws`.

MQTT may be considered later for large-scale presence or event fanout, but it is
not the first control-channel choice. WebSocket keeps the Agent registration,
task lease, confirmation, cancellation, and Core event mapping in one typed
application protocol.

## Rust Client Libraries

The Agent CLI is Rust-based. The recommended client stack is:

| Concern | Recommended crate |
| --- | --- |
| Async runtime | `tokio` |
| WebSocket client | `tokio-tungstenite` |
| Stream and sink split | `futures` |
| Serialization | `serde`, `serde_json` |
| IDs | `uuid` |
| Cancellation | `tokio-util` cancellation tokens |
| Logging | `tracing` |
| HTTP bootstrap or fallback | `reqwest` |

Use rustls-compatible dependency features for cross-platform release builds.
The existing workspace already uses `tokio`, `tokio-util`, `futures`, `serde`,
`serde_json`, `uuid`, `tracing`, and `reqwest` with rustls.

## FastAPI Backend Compatibility

The backend may be implemented with FastAPI. The WebSocket endpoint can share
the same service port as normal REST APIs:

```text
GET  /api/agent/ws
GET  /api/team/agents
POST /api/team/tasks
GET  /api/team/tasks/{task_id}
```

The Agent CLI must treat the FastAPI WebSocket endpoint as the authoritative
team coordination endpoint, but local execution remains inside the local Alius
runtime. The backend may assign tasks and send control commands; it must not
directly execute local shell, filesystem, plugin, or tool operations.

## Connection Handshake

The WebSocket upgrade request should include authentication and protocol
selection:

```text
GET /api/agent/ws
Authorization: Bearer <agent-token>
X-Agent-Id: <agent-id>
X-Workspace-Id: <workspace-id>
Sec-WebSocket-Protocol: alius-team.v1
```

The first application message after upgrade must still be `RegisterAgent`.
Connection-level authentication is not enough because the backend must bind the
connection to the current Agent identity, workspace, protocol version, and
declared capabilities.

Example:

```json
{
  "type": "register_agent",
  "protocol_version": "alius-team.v1",
  "message_id": "msg-001",
  "agent_id": "agent-001",
  "instance_id": "macbook-local-001",
  "workspace_id": "workspace-abc",
  "soul": "rust-reviewer",
  "role": "reviewer",
  "capabilities": ["join_team", "receive_task", "use_model", "read_workspace"],
  "last_seen_seq": 0
}
```

The backend returns granted capabilities. It must not trust the Agent CLI's
self-declared capabilities as final authorization:

```json
{
  "type": "register_ack",
  "protocol_version": "alius-team.v1",
  "connection_id": "conn-123",
  "agent_id": "agent-001",
  "granted_capabilities": ["join_team", "receive_task", "use_model"],
  "denied_capabilities": ["use_shell", "write_config"]
}
```

## Message Envelope

Every Agent Team message must include stable routing and replay fields:

```json
{
  "protocol_version": "alius-team.v1",
  "message_id": "msg-123",
  "connection_id": "conn-123",
  "agent_id": "agent-001",
  "team_id": "team-001",
  "workspace_id": "workspace-abc",
  "trace_id": "trace-abc",
  "seq": 42,
  "timestamp": "2026-06-18T09:56:00Z",
  "type": "heartbeat",
  "payload": {}
}
```

`seq` is scoped to the Agent connection stream. The Agent CLI sends
`last_seen_seq` on reconnect so the backend can replay missed durable events or
instruct the Agent to resync.

## Required Message Types

Agent CLI to backend:

- `RegisterAgent`
- `Heartbeat`
- `StatusUpdate`
- `RunEvent`
- `TaskAccepted`
- `TaskRejected`
- `TaskProgress`
- `TaskResult`
- `ConfirmationRequired`
- `ErrorReport`

Backend to Agent CLI:

- `RegisterAck`
- `TaskOffer`
- `CancelRun`
- `PauseRun`
- `ResumeRun`
- `ConfirmationDecision`
- `SyncRequest`
- `ConfigUpdate`
- `ShutdownNotice`
- `Error`

Unknown message types must be rejected with a structured error and must not
close the connection unless the message violates protocol or security policy.

## Presence and Heartbeat

The Agent CLI should send a heartbeat every 5 seconds while connected:

```json
{
  "type": "heartbeat",
  "agent_id": "agent-001",
  "seq": 128,
  "presence": "online",
  "work_status": "running_tool",
  "run_ref": "run-abc",
  "pending_confirmations": 0
}
```

Backend state transitions:

| Condition | Presence |
| --- | --- |
| Connection established and registered | `online` |
| Reconnect or event catch-up in progress | `syncing` |
| No heartbeat for 15 seconds | `degraded` |
| No heartbeat for 30-45 seconds or socket closed | `offline` |

Presence and work status must remain separate. An Agent can be online while
waiting for approval, and it can be degraded while the backend still has a
non-terminal task lease to reconcile.

## Work Status

The first implementation should support these work states:

```text
idle
planning
running
streaming
waiting_for_approval
running_tool
reviewing
blocked
completed
failed
cancelled
```

The Agent CLI derives work status from local Core Runtime events where possible.
For example, `ToolConfirmationRequired` maps to `waiting_for_approval`, tool
start maps to `running_tool`, and final results map to `completed` or `failed`.

## Task Lease

Task assignment must be lease-based:

```text
TaskOffer -> TaskAccepted(lease_id) -> RunEvent* -> TaskResult
```

Rules:

- The backend must include a `task_id` and lease TTL in each task offer.
- The Agent CLI must return `TaskAccepted` before executing.
- The Agent CLI must renew the lease while work is active.
- The backend must not assign the same active task to multiple Agents unless the
  task is explicitly marked as parallel.
- If the Agent disconnects past the lease TTL, the backend may mark the task as
  lost, failed, or eligible for reassignment.

## Permission Model

WebSocket does not provide business permissions. Agent Team permissions must be
implemented in the application protocol:

- Authenticate the WebSocket upgrade request.
- Register and authorize the Agent after upgrade.
- Treat self-declared capabilities as requested capabilities only.
- Validate every control message against `agent_id`, `team_id`, `workspace_id`,
  granted capabilities, and active lease.
- Never let the backend silently approve local high-risk operations.
- Keep local tool, shell, filesystem, plugin, and confirmation policy inside the
  Agent CLI and Core Runtime.
- Audit task assignment, cancellation, confirmation decisions, failures, and
  permission denials.

Recommended capability names:

```text
join_team
receive_task
delegate_task
use_model
read_workspace
write_workspace
use_tools
use_shell
use_mcp
read_memory
write_memory
approve_remote_confirmation
```

`approve_remote_confirmation` must be denied by default. Remote confirmation
support may forward a user's explicit backend decision, but it must not bypass
the local runtime's confirmation gate.

## Reconnect and Replay

The Agent CLI must reconnect with exponential backoff and jitter. On reconnect,
it sends:

- `agent_id`
- new connection attempt metadata
- `last_seen_seq`
- current local run status if a run is still active

The backend can then respond with:

- `RegisterAck` if the stream can resume;
- `SyncRequest` if the Agent must send a full status snapshot;
- `TaskOffer` or `CancelRun` only after the connection is authorized again.

Old `connection_id` values must not grant authority after reconnect. Every
connection receives a new `connection_id`.

## TUI Integration

The TUI must continue to keep local Conversation separate from Agent Team
traffic. Agent Team events should populate `AgentTeamState` and render in the
Agent Team tab with direction (`IN` or `OUT`), sender, receiver, type, status,
and content summary.

The local Conversation area should show only local user/runtime workflow unless
a Team event is intentionally surfaced as a local status block.

## Acceptance Criteria

- Agent CLI connects outbound to a FastAPI WebSocket endpoint without opening a
  local inbound port.
- The connection authenticates during upgrade and registers through a first
  application message.
- Backend returns granted capabilities; Agent CLI stores them as the active
  connection authorization.
- Heartbeat updates presence and work status.
- Missing heartbeat produces degraded/offline transitions on the backend.
- Task execution requires `TaskOffer` and `TaskAccepted` with `lease_id`.
- Backend control messages are rejected when the lease, workspace, team, or
  capability does not match.
- Core Runtime events are mapped into Agent Team status and event messages.
- Reconnect uses a new connection id and carries `last_seen_seq`.
- TUI Agent Team state is populated only by real Agent Team events, not by local
  Conversation messages.
- Tests cover handshake rejection, successful registration, heartbeat, degraded
  timeout, task lease acceptance, unauthorized command rejection, reconnect
  replay request, and TUI state population.
