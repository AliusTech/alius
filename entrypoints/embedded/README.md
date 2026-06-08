# Alius Embedded SDK

C/C++ FFI interface for integrating Alius LLM chat into embedded systems.

## Phase 1: ESP32 with FreeRTOS

Current implementation targets ESP32 microcontrollers running FreeRTOS.

## Features

- ✅ C and C++ API
- ✅ Streaming chat responses
- ✅ Single-session design (simplified for embedded)
- ✅ Minimal dependencies
- ✅ ESP-IDF integration
- ✅ FreeRTOS task support

## Quick Start

### Building

```bash
# Build for desktop platforms (macOS/Linux/Windows)
cargo build --release --features std

# Build for embedded (no_std)
cargo build --release --features embedded

# Build for ESP32 hardware (requires ESP toolchain)
rustup target add xtensa-esp32-espidf
cargo build --release --features esp32 --target xtensa-esp32-espidf
```

**✅ Note:** The `esp32` feature now works on all platforms:
- **Desktop platforms**: Compiles with embedded features (no ESP-IDF)
- **ESP targets**: Compiles with full ESP-IDF support

### Supported ESP Targets

**Xtensa Architecture:**
- `xtensa-esp32-espidf` - ESP32
- `xtensa-esp32s2-espidf` - ESP32-S2
- `xtensa-esp32s3-espidf` - ESP32-S3

**RISC-V Architecture:**
- `riscv32imc-esp-espidf` - ESP32-C2, ESP32-C3
- `riscv32imac-esp-espidf` - ESP32-C5, ESP32-C6, ESP32-H2, ESP32-P4

**Experimental:**
- ESP32-S31 (RISC-V) - Target name TBD, requires ESP-IDF master

### Dependencies

The `embedded` feature excludes:
- ❌ TUI (ratatui, crossterm)
- ❌ CLI tools
- ❌ WASM runtime (wasmtime)
- ❌ Desktop-only features

The `embedded` feature includes:
- ✅ Core protocol interface
- ✅ LLM client and streaming
- ✅ Network (HTTP/TLS)
- ✅ Configuration management

## API Examples

### C API

```c
#include "alius.h"

// Initialize
alius_init();

// Configure
alius_config_set_model("anthropic", "claude-haiku-4-20250218", "api-key");

// Chat with streaming
alius_chat("Hello!", on_delta, on_error, NULL);

// Cleanup
alius_cleanup();
```

### C++ API

```cpp
#include "alius.hpp"

// Initialize
alius::EmbeddedClient::init();

// Configure
alius::ModelConfig config;
config.provider = "anthropic";
config.model = "claude-haiku-4-20250218";
config.api_key = "your-api-key";
alius::EmbeddedClient::configureModel(config);

// Chat
alius::StreamCallbacks cb;
cb.on_delta = [](const std::string& delta) {
    printf("%s", delta.c_str());
};
alius::EmbeddedClient::chat("Hello from ESP32!", cb);
```

## Examples

See `examples/esp32/` for ESP32 with FreeRTOS examples.

## Hardware Requirements

### ESP32 (Phase 1)
- **Recommended**: ESP32-WROVER or ESP32-S3 (more RAM)
- **Minimum**: ESP32 with WiFi
- **Connectivity**: WiFi (2.4GHz)
- **RAM**: ~280KB DRAM typical usage

### ESP32-S31 (Experimental)
- **Status**: 🧪 Experimental support (Alpha)
- **Architecture**: Dual-core RISC-V (320MHz)
- **Connectivity**: Wi-Fi 6, Bluetooth 5.4, Thread (802.15.4)
- **Requirements**: ESP-IDF master branch
- **Note**: Rust target support pending upstream definition

## Architecture

```
┌─────────────────────────────────────┐
│   Application (C/C++)                │
├─────────────────────────────────────┤
│   Alius Embedded SDK (FFI)          │
│   ├── runtime.rs (FFI functions)    │
│   ├── stream.rs (streaming)          │
│   └── error.rs (error handling)     │
├─────────────────────────────────────┤
│   Core Runtime                       │
│   ├── ProtocolInterface              │
│   ├── CoreRuntime                    │
│   └── LLM Client                     │
├─────────────────────────────────────┤
│   Platform Layer                     │
│   ├── ESP-IDF / FreeRTOS            │
│   └── WiFi / Networking             │
└─────────────────────────────────────┘
```

## Build Features

| Feature | Description | Desktop | ESP Hardware |
|---------|-------------|---------|--------------|
| `default` | Standard library support | ✅ | ❌ |
| `std` | Enable std library | ✅ | ❌ |
| `embedded` | Minimal embedded build | ✅ | ✅ |
| `esp32` | ESP32 features (ESP-IDF on ESP targets) | ✅* | ✅ |
| `full` | All features (desktop) | ✅ | ❌ |

*On desktop platforms, `esp32` feature compiles without ESP-IDF dependencies.

## Configuration

### Model Selection

For ESP32 resource constraints:

1. **Claude Haiku** - Recommended for ESP32
   - Fastest
   - Lowest memory
   - Simple tasks

2. **Claude Sonnet** - For ESP32-WROVER/S3
   - Better quality
   - Higher memory
   - Complex tasks

### Memory Optimization

```toml
# .cargo/config.toml
[build]
opt-level = "s"  # Optimize for size
```

## Future Phases

- **Phase 1.5**: ESP32-S31 support stabilization
  - ⏳ Waiting for stable ESP-IDF release
  - ⏳ Waiting for Rust target definition
  - ⏳ esp-idf-sys compatibility verification
- **Phase 2**: STM32 + RTOS support
- **Phase 3**: Linux host mode for UART bridge
- **Future**: Direct bare-metal port with `no_std`

## License

MIT

## Support

For issues and questions, see the main Alius repository.
