# Diagrams

This file is the maintained Mermaid source for high-level workspace diagrams.

## Main Architecture

```mermaid
flowchart TB
  subgraph Product["Product Entrypoints"]
    CLI["alius-cli\nCLI and TUI"]
    JSONRPC["jsonrpc\nlightweight adapter"]
  end

  subgraph Protocol["Protocol Interface"]
    CONTRACT["protocol-interface\ncontracts and envelope"]
    BRIDGE["ProtocolBridge\nCLI friendly wrapper"]
  end

  subgraph Core["Core Runtime"]
    MANAGER["CoreRuntimeManager\nlocal runtime assembly"]
    RUNTIME["CoreRuntime"]
    SESSION["SessionManager"]
    LOOP["LoopEngine"]
  end

  subgraph Subsystems["Runtime Subsystems"]
    CONFIG["runtime-config"]
    MODEL["runtime-model"]
    TOOLS["runtime-tools"]
    STORE["runtime-store"]
  end

  CLI --> BRIDGE
  JSONRPC --> MANAGER
  BRIDGE --> MANAGER
  MANAGER --> CONTRACT
  CONTRACT --> RUNTIME
  RUNTIME --> SESSION
  RUNTIME --> LOOP
  RUNTIME --> CONFIG
  LOOP --> MODEL
  LOOP --> TOOLS
  RUNTIME --> STORE
```

## Request and Event Flow

```mermaid
sequenceDiagram
  participant Product
  participant Manager as CoreRuntimeManager
  participant Protocol as Protocol Interface
  participant Core as CoreRuntime
  participant Session as SessionManager
  participant Loop as LoopEngine
  participant Model as LlmClient

  Product->>Manager: run_text or start_streaming
  Manager->>Protocol: ProtocolEnvelope<CoreRequest>
  Protocol->>Protocol: validate version and capability ceiling
  Protocol->>Core: start request
  Core->>Session: create session or turn
  Core->>Loop: run Chat, Bypass, or Plan mode
  Loop->>Model: model request
  Model-->>Loop: model deltas
  Loop-->>Core: CoreEvent values
  Core-->>Protocol: run ref and events
  Protocol-->>Product: CoreEvent stream or wrapped events
```

## Project State

```mermaid
flowchart LR
  ROOT["Project root"] --> ALIUS[".alius"]
  ALIUS --> CONFIG["config\nruntime configuration"]
  ALIUS --> MEMORY["memory\nruntime data"]
  ALIUS --> WORKSPACE["workspace\nauthoritative docs"]
  MEMORY --> COMM["communications/sessions"]
  MEMORY --> LOGS["logs"]
  MEMORY --> DESIGN["design\nhistorical input"]
```
