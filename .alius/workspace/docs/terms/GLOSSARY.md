# Glossary

## Agent Card

Project-level description of an Alius agent identity, capabilities, skills, and protocol exposure. The project source is represented through `.alius/config/soul.toml`.

## Agent CLI

The local Rust CLI process that represents an Agent in Team mode. It initiates an outbound WebSocket connection to the Agent Team Backend and keeps local runtime execution, permissions, confirmations, and audit under local control.

## Agent Connection

One long-lived WebSocket session between an Agent CLI and the Agent Team Backend. A connection has its own connection id, heartbeat stream, authorization state, and sequence position.

## Agent Presence

The connection-level status of an Agent CLI in Team mode, such as connecting, online, syncing, degraded, reconnecting, or offline.

## Agent Team Backend

The server-side coordination API for Agent Team mode. It maintains Agent registration, presence, work status, task leases, and team event streams.

## Agent Task Lease

A backend-issued ownership record that allows one Agent CLI to execute a task for a bounded time window.

## Agent Work Status

The execution-level status of an Agent CLI, such as idle, planning, running, streaming, waiting for approval, running tool, reviewing, blocked, completed, failed, or cancelled.

## Bypass Mode

TUI mode that submits input directly for execution without first guiding the user through local plan review.

## Capability Scope

The capability ceiling supplied by the product or adapter origin in `ProtocolEnvelope<T>`. It is not the final authorization decision.

## Core Command

A control message sent to a running Core run. Examples include cancel, approve, deny, pause, continue, and mode switching.

## Core Event

An event emitted by Core Runtime to describe run progress, model deltas, tool activity, errors, and final results.

## Core Request

A request that starts or inspects work in Core Runtime. Examples include run loop, start turn, open session, list sessions, memory, config, and health operations.

## Core Runtime

The unified execution layer implemented by `core-runtime`. It owns session lifecycle, loop execution, event adaptation, logging helpers, and runtime state.

## Loop Engine

The module that executes Chat, Bypass, and Plan modes through one orchestration path controlled by `LoopPolicy`.

## Bypass Permissions

A high-risk `LoopPolicy.permission_strategy` value that skips Alius confirmation and permission interception for the current execution path. It does not bypass operating-system permissions, missing paths, failed process execution, network failures, or tool implementation errors.

## Origin

The product or adapter identity that submitted a protocol message, such as `LocalCli`, `LocalTui`, `JsonRpc`, `RemoteA2A`, or `PluginRpc`.

## Plan Mode

TUI mode oriented around goal understanding, plan nodes, execution, review, evidence, and final result handling.

## Plans

The TUI panel and state model for plan nodes, statuses, ownership, acceptance criteria, and evidence.

## Product Entrypoint

A user-facing or integration-facing entrypoint such as CLI, TUI, JSON-RPC, desktop, IDE extension, or remote agent protocol.

## Protocol Envelope

The shared wrapper for protocol messages. It carries version, origin, capability scope, workspace root, session reference, run reference, trace id, and payload.

## Protocol Interface

The boundary between product entrypoints and Core Runtime. It validates envelopes, normalizes capability ceilings, delegates to Core Runtime, and wraps events.

## Run

One execution instance inside a session. A run has a `RunRef`, status, associated events, and trace identity.

## Session

A resumable workspace context that groups runs and turns. It is represented by `SessionRef` in the protocol layer.

## Shell Gate

The command inspection and authorization subsystem for shell, process, and git-like operations.

## Trace

The identifier chain that connects requests, commands, events, logs, and runtime observations for a run.

## Workspace

The project root managed by Alius. Project config, memory, and workspace documentation live under `.alius/`.
