# Isochron Display Firmware

Firmware for the V0 Mini OLED display module (STM32F042K6). This runs on the display's dedicated MCU and communicates with the main controller via UART.

## Hardware

- **MCU**: STM32F042K6 (Cortex-M0, 32KB Flash, 6KB RAM)
- **Display**: SH1106 128x64 OLED (I2C)
- **Input**: Rotary encoder with push button

### Pin Assignments

| Function | Pin | Notes |
|----------|-----|-------|
| I2C SCL | PB6 | OLED display |
| I2C SDA | PB7 | OLED display |
| UART TX | PA2 | To controller RX |
| UART RX | PA3 | From controller TX |
| Encoder A | PA4 | Quadrature input |
| Encoder B | PA5 | Quadrature input |
| Encoder Button | PA1 | Active low, with pullup |

## Building

From the display firmware directory:

```bash
cd displays/isochron-display-fw
cargo build --release
```

Or using the Makefile from the workspace root:

```bash
make profile PROFILE=btt-pico  # Includes display firmware
make build
```

## Flashing

### Using probe-rs

```bash
probe-rs run --chip STM32F042K6Tx target/thumbv6m-none-eabi/release/isochron-display-fw
```

### Using ST-Link

Connect ST-Link to the SWD header and use your preferred flashing tool.

## Protocol

The display firmware uses the `isochron-protocol` crate for communication. It receives commands from the main controller and sends input events back.

### Received Commands (from controller)

| Command | Description |
|---------|-------------|
| `Ping` | Heartbeat check |
| `ClearScreen` | Clear all text |
| `Text { row, col, text }` | Draw text at position |
| `Invert { row, start, end }` | Invert region (for selection highlight) |
| `Reset` | Reset display state |

### Sent Events (to controller)

| Event | Description |
|-------|-------------|
| `Ping` | Heartbeat response (every 1s) |
| `Input(EncoderCW)` | Encoder rotated clockwise |
| `Input(EncoderCCW)` | Encoder rotated counter-clockwise |
| `Input(EncoderClick)` | Button short press |
| `Input(EncoderLongPress)` | Button held > 500ms |

## Architecture

The firmware uses Embassy async runtime with the following tasks:

| Task | Purpose |
|------|---------|
| `uart_rx_task` | Receives and parses controller commands |
| `uart_tx_task` | Sends input events and heartbeats |
| `encoder_task` | Polls quadrature encoder for rotation |
| `button_task` | Handles button press/long-press detection |
| `display_task` | Renders display state to OLED |

## Memory Usage

The STM32F042K6 has limited resources:
- 32KB Flash
- 6KB RAM

Release builds with LTO typically use:
- ~28KB Flash
- ~3KB RAM

## Related Crates

- `isochron-hal-stm32f0` - STM32F0 HAL implementation
- `isochron-protocol` - UART protocol definitions
- `isochron-display` - Display traits and rendering utilities
