# Isochron

> **Warning**
> This is **alpha software** under active development. APIs, configuration formats, and behavior may change without notice. Not recommended for production use.

Embedded firmware for automated watch cleaning machines, running on RP2040-based boards (BTT SKR Pico).

Named after the Greek "isochronous" (equal time), reflecting the precision timing of watch movements.

## Features

- **Klipper-inspired configuration** - All hardware defined in TOML config, not code
- **Embassy async runtime** - Efficient cooperative multitasking
- **TMC2209 integration** - Silent stepper control with StallGuard stall detection
- **Safety monitoring** - Over-temperature, motor stall, communication fault detection
- **V0 Display support** - OLED display with rotary encoder interface

## Project Structure

```
isochron/
├── isochron-core/           # Board-agnostic application logic
├── isochron-drivers/        # Hardware driver implementations (TMC2209, etc.)
├── isochron-hal-rp2040/     # RP2040-specific HAL
├── isochron-protocol/       # Display communication protocol
├── isochron-firmware/       # Main firmware binary
└── docs/                    # Documentation
```

## Quick Start

See [docs/](docs/) for full documentation:

- [Installation Guide](docs/Installation.md)
- [Configuration Reference](docs/Config_Reference.md)
- [Supported Boards](docs/Boards.md)

## Building

```bash
# Install Rust embedded target
rustup target add thumbv6m-none-eabi

# Build firmware
cd isochron-firmware
cargo build --release
```

## License

This project is licensed under the **GNU General Public License v3.0 or later** (GPL-3.0-or-later).

This means:
- You can use, modify, and distribute this software
- If you distribute devices with this firmware, you must provide source code and allow users to install modified versions
- Derivative works must also be licensed under GPLv3

See [LICENSE](LICENSE) for the full license text.
