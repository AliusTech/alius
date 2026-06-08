# Model Runtime Module

Primary paths:

- `runtime/model/src/client.rs`
- `runtime/model/src/provider.rs`
- `runtime/model/src/openai_provider.rs`
- `runtime/model/src/anthropic_provider.rs`
- `runtime/model/src/google_provider.rs`
- `runtime/model/src/conversation.rs`
- `runtime/model/src/agent.rs`
- `runtime/model/src/router.rs`
- `runtime/model/src/credential.rs`

## Responsibilities

- Provide `LlmClient`.
- Define provider abstraction.
- Normalize streaming and non-streaming model responses.
- Manage conversation state.
- Support tool calling paths.
- Provide agent loop events and execution.
- Resolve credentials.
- Provide model router types and strategy.

## Provider State

| Provider path | Current documentation status |
| --- | --- |
| OpenAI-compatible | Implemented path for OpenAI-style APIs. |
| BigModel | Uses OpenAI-compatible behavior. |
| Custom | Uses OpenAI-compatible behavior. |
| Anthropic | Native provider path exists. |
| Google | Code exists, but should be verified before calling it production-complete. |

## Conversation

`Conversation` tracks:

- system prompt
- messages
- summary
- max token estimate

Summarization and context compaction should be documented only where they are connected to the path being described.

## Agent Loop

`AliusAgent` exists for tool-calling behavior, but product paths should be checked before claiming that every default workspace response uses the agent loop.

