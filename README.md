# Isochron

> **Warning**
> This is **alpha software** under active development. APIs, configuration formats, and behavior may change without notice. Not recommended for production use.

Embedded firmware for automated watch cleaning machines, supporting RP2040-based boards with a modular, Klipper-inspired architecture.

Named after the Greek "isochronous" (equal time), reflecting the precision timing of watch movements.

## Features

- **Klipper-style build system** - `make menuconfig` with profiles for quick board switching
- **Modular HAL architecture** - Support multiple MCU families (RP2040, STM32)
- **Embassy async runtime** - Efficient cooperative multitasking
- **Multiple motor types** - Stepper (TMC2209), DC (PWM), and AC (relay) motor support
- **Safety monitoring** - Over-temperature, motor stall, communication fault detection
- **Display abstraction** - Support for external (V0) and direct-connected displays

## Project Structure

```
isochron/
├── hal/                         # Hardware Abstraction Layers
│   ├── isochron-hal/           # Shared traits (chip-agnostic)
│   ├── isochron-hal-rp2040/    # RP2040 implementation (controller)
│   └── isochron-hal-stm32f0/   # STM32F0 implementation (display)
├── displays/                    # Display modules
│   ├── isochron-display/       # Display traits + shared code
│   └── isochron-display-fw/    # V0 Mini display firmware (STM32F042)
├── isochron-core/              # Core logic (config, cycles, PID)
├── isochron-drivers/           # Device drivers (motors, heaters)
├── isochron-protocol/          # UART protocol
├── isochron-firmware/          # Main controller firmware
├── configs/                     # Configuration files
│   ├── boards/                 # Board pin mappings
│   └── machines/               # Machine configurations
├── profiles/                    # Build profiles (like Klipper)
├── Kconfig                     # Build configuration schema
└── Makefile                    # Build system
```

## Quick Start

### Prerequisites

```bash
# Rust embedded target
rustup target add thumbv6m-none-eabi

# Build tools
pip install kconfiglib           # For menuconfig
cargo install elf2uf2-rs         # For UF2 generation
```

### Build

```bash
# Option 1: Use a pre-defined profile
make profile PROFILE=btt-pico
make build

# Option 2: Interactive configuration
make menuconfig
make build

# Flash (drag-and-drop or probe-rs)
make flash
# OR copy out/isochron-firmware.uf2 to BOOTSEL drive
```

### Profiles

Switch between board configurations easily:

```bash
make list-profiles              # Show available profiles
make profile PROFILE=btt-pico   # Load shipped profile
make save-profile PROFILE=my-setup  # Save your configuration
```

## Documentation

- [Architecture Overview](docs/Architecture.md)
- [Hardware Support](docs/Hardware_Support.md)
- [Configuration Reference](docs/Config_Reference.md)

## Supported Hardware

| Board | Status |
|-------|--------|
| BTT SKR Pico | Supported |
| Raspberry Pi Pico | Supported |
| Adafruit Feather RP2040 | Compatible |
| Other RP2040 boards | Via custom config |

See [Hardware Support](docs/Hardware_Support.md) for full details.

## License

This project is licensed under the **GNU General Public License v3.0 or later** (GPL-3.0-or-later).

This means:
- You can use, modify, and distribute this software
- If you distribute devices with this firmware, you must provide source code and allow users to install modified versions
- Derivative works must also be licensed under GPLv3

See [LICENSE](LICENSE) for the full license text.
