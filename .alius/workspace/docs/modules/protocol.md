# Protocol Module

Primary paths:

- `protocol/src/core.rs`
- `protocol/src/interface.rs`
- `protocol/src/error.rs`
- `protocol/src/types.rs`
- `protocol/src/message.rs`

## Responsibilities

- Define stable protocol contracts.
- Define shared ids and references.
- Define protocol errors.
- Define `CoreRuntimeApi`.
- Provide `ProtocolInterface<R>` as a Direct Rust API gateway.

## Main Contracts

- `ProtocolEnvelope<T>`
- `Origin`
- `CapabilityScope`
- `Capability`
- `CoreRequest`
- `CoreCommand`
- `CoreEvent`
- `ProtocolError`
- `CoreRuntimeApi`

## Gateway Behavior

`ProtocolInterface<R>`:

1. Validates protocol version.
2. Validates origin capability ceiling for selected operations.
3. Delegates to `CoreRuntimeApi`.
4. Stores run context.
5. Wraps subscribed events with their original protocol context.

## Boundary Rules

- Protocol types should not depend on TUI-specific state.
- Protocol Interface should not implement model provider calls.
- Protocol Interface should not implement tool behavior.
- Product adapters should normalize into protocol contracts.

