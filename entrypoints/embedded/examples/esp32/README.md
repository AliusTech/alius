# Alius Embedded SDK - ESP32 Example

## Hardware Requirements

### Recommended
- **ESP32-WROVER** - More RAM (4MB PSRAM)
- **ESP32-S3** - Better performance and more resources

### Minimum
- **ESP32** - Standard ESP32 with WiFi (limited resources)

## Wiring/Setup

### ESP32-WROVER / ESP32-S3
```
USB Programming
├── USB → UART (built-in)
└── Power (5V from USB)

WiFi
└── Built-in WiFi antenna

Debug (Optional)
├── GPIO2 → LED (for status indication)
└── GPIO4/Button → User input
```

### Connections
1. Connect ESP32 to computer via USB
2. Install ESP-IDF toolchain
3. Configure WiFi credentials in code

## Setup Instructions

### 1. Install ESP-IDF

```bash
# Clone ESP-IDF
git clone --recursive https://github.com/espressif/esp-idf.git ~/esp/esp-idf
cd ~/esp/esp-idf
git checkout v5.1.2

# Install toolchain
./install.sh esp32
source ./export.sh
```

### 2. Install Rust ESP32 Target

```bash
rustup target add xtensa-esp32-espidf
rustup target add xtensa-esp32s3-espidf

# Install espflash
cargo install espflash
```

### 3. Configure the Example

Edit `freertos_chat.cpp` and update:

```cpp
#define WIFI_SSID "your-wifi-ssid"
#define WIFI_PASS "your-wifi-password"
#define ALIUS_API_KEY "your-api-key-here"
```

### 4. Build and Flash

```bash
cd entrypoints/embedded

# Build for ESP32
cargo build --release --features esp32 --target xtensa-esp32-espidf

# Flash to device
cargo espflash flash --release --features esp32

# Monitor serial output
cargo espflash monitor
```

## Expected Output

```
I (1234) ALIUS_EXAMPLE: Alius Embedded SDK Example
I (1235) ALIUS_EXAMPLE: Chip: esp32, 2 CPU cores, 240 MHz
I (1236) ALIUS_EXAMPLE: Alius SDK Version: 0.1.0
I (1237) ALIUS_EXAMPLE: Initializing WiFi...
I (2345) ALIUS_EXAMPLE: Got IP:192.168.1.100
I (3456) ALIUS_EXAMPLE: Starting chat task...
I (3457) ALIUS_EXAMPLE: Alius initialized
I (3458) ALIUS_EXAMPLE: Health check passed
I (3459) ALIUS_EXAMPLE: Model configured
I (3460) ALIUS_EXAMPLE: Starting chat...
I (3461) ALIUS_EXAMPLE: Chat initiated successfully
I (5000) ALIUS_EXAMPLE: Delta: Hello
I (5001) ALIUS_EXAMPLE: Delta:  from
I (5002) ALIUS_EXAMPLE: Delta:  the
I (5003) ALIUS_EXAMPLE: Delta:  AI
I (5100) ALIUS_EXAMPLE: Delta: !
```

## Troubleshooting

### Build Errors

**Error: `xtensa-esp32-espidf` not found**
```bash
rustup target add xtensa-esp32-espidf
```

**Error: ESP-IDF not found**
```bash
source ~/esp/esp-idf/export.sh
```

### Runtime Errors

**WiFi not connecting**
- Check SSID and password in code
- Ensure WiFi is in range
- Check router is 2.4GHz (ESP32 doesn't support 5GHz)

**API errors**
- Verify API key is valid
- Check network connectivity
- Ensure Haiku model is accessible

**Memory issues**
- Use ESP32-WROVER with PSRAM
- Reduce WiFi buffer size
- Close unnecessary connections

## Memory Usage

Typical memory usage for ESP32:

| Component | DRAM | PSRAM |
|-----------|------|-------|
| SDK Core | ~50KB | - |
| WiFi Stack | ~80KB | - |
| Alius Runtime | ~100KB | - |
| Chat Request | ~20KB | - |
| Response Buffer | ~30KB | - |
| **Total** | **~280KB** | **Optional** |

For ESP32-S3 with PSRAM, additional memory can be used for:
- Larger response buffers
- Multiple concurrent requests
- Response caching

## Model Recommendations

For ESP32 resource constraints:

1. **Claude Haiku** (`claude-haiku-4-20250218`)
   - Fastest responses
   - Lower memory footprint
   - Good for simple tasks

2. **Claude Sonnet** (`claude-sonnet-4-20250218`)
   - Better quality responses
   - Higher memory usage
   - Slower responses

For ESP32-WROVER/S3 with PSRAM, Sonnet is recommended.

## Next Steps

1. **Custom Responses**: Modify the chat prompt for your use case
2. **Display Integration**: Connect an LCD/OLED for response display
3. **Voice Input**: Add microphone for voice-to-text
4. **Multi-turn Conversations**: Implement conversation history
5. **Offline Mode**: Cache common responses
