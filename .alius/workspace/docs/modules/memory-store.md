# Memory Store Module

Primary paths:

- `runtime/memory/src/memory.rs`
- `runtime/memory/src/conversation.rs`
- `runtime/memory/src/session.rs`
- `runtime/memory/src/episodic/`
- `runtime/memory/src/semantic/`
- `runtime/memory/src/procedural/`
- `runtime/memory/src/retrieval/`
- `runtime/memory/src/paths.rs`

## Responsibilities

- Store project and global memory data.
- Store conversations.
- Store sessions.
- Represent episodic, semantic, and procedural memory.
- Provide retrieval behavior.
- Resolve memory paths.

## Main Types

- `MemoryStore`
- `ConversationStore`
- `SessionStore`
- `EpisodicStore`
- `SemanticStore`
- `ProceduralStore`
- `RetrievalEngine`

## Runtime Memory vs Workspace Docs

Runtime memory lives under `.alius/memory/`.

Workspace documentation lives under `.alius/workspace/`.

Historical generated design memory under `.alius/memory/design/` can be used as migration input, but new authoritative documentation belongs in `.alius/workspace/`.

## Store Boundary

Store modules should not know TUI layout, provider-specific request formats, or command parsing details.

## Known Gaps

Before documenting persistence as complete, verify whether the path stores process memory only, JSON files, SQLite files, or other backing stores for the specific feature.

