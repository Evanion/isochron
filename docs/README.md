# Isochron Documentation

Isochron is embedded firmware for automated watch cleaning machines, running on RP2040-based boards.

## Quick Start

1. [Installation Guide](Installation.md) - Get up and running
2. [Configuration Reference](Config_Reference.md) - Customize your machine
3. [Supported Boards](Boards.md) - Hardware options

## Documentation Overview

### For Users

| Document | Description |
|----------|-------------|
| [Installation Guide](Installation.md) | Install tools, build firmware, flash to board |
| [Configuration Reference](Config_Reference.md) | All configuration options (Klipper-style) |
| [Supported Boards](Boards.md) | Hardware compatibility and pinouts |

### For Developers

| Document | Description |
|----------|-------------|
| [Developer Guide](DEVELOPER.md) | Debug probes, RTT logging, GDB debugging |
| [Architecture](ARCHITECTURE.md) | Crate structure, task system, state machine |

### Design Documents

| Document | Description |
|----------|-------------|
| [Implementation Plan](implementation-plan.md) | Original design and implementation phases |

---

## What is Isochron?

Isochron is firmware for controlling watch part cleaning machines. Named after the Greek "isochronous" (equal time), reflecting the precision timing of watch movements.

### Features

- **Klipper-inspired configuration**: All hardware defined in config, not code
- **Multiple machine types**: Manual or fully automated
- **Safety monitoring**: Over-temp, motor stall, sensor fault detection
- **V0 Display support**: OLED display with encoder for user interface
- **TMC2209 integration**: Silent stepper control with StallGuard

### Machine Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Watch Cleaning Machine                   │
│                                                              │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐  │
│   │  Clean  │    │ Rinse 1 │    │ Rinse 2 │    │   Dry   │  │
│   │   Jar   │    │   Jar   │    │   Jar   │    │ Chamber │  │
│   └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘  │
│        │              │              │              │        │
│        └──────────────┼──────────────┼──────────────┘        │
│                       │              │                       │
│                 ┌─────┴──────────────┴─────┐                │
│                 │         Basket           │ ← spin motor   │
│                 │    (single, moves)       │                │
│                 └──────────────────────────┘                │
└─────────────────────────────────────────────────────────────┘
```

A watch cleaning machine has a single basket that moves through multiple jars:
1. **Clean** - Cleaning solution with agitation
2. **Rinse** - One or more rinse baths
3. **Dry** - Heated drying chamber

### Supported Hardware

- **Primary**: BTT SKR Pico (RP2040 + 4x TMC2209)
- **Display**: Voron V0 Display (OLED + rotary encoder)
- **Motors**: Standard NEMA17 steppers
- **Heaters**: SSR or MOSFET controlled

---

## Project Structure

```
chronohub/cleaner/
├── docs/                    # Documentation (you are here)
├── isochron-core/           # Board-agnostic application logic
├── isochron-drivers/        # Hardware driver implementations
├── isochron-hal-rp2040/     # RP2040-specific HAL
├── isochron-protocol/       # Display communication protocol
└── isochron-firmware/       # Main firmware binary
```

---

## Getting Help

- **Issues**: Report bugs on GitHub
- **Discussions**: Ask questions on GitHub Discussions

---

## License

This project is licensed under the GNU General Public License v3.0 or later - see [LICENSE](../LICENSE) for details.
