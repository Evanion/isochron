# isochron-hal-rp2040

RP2040 implementation of the Isochron HAL traits.

This crate provides hardware abstraction for RP2040-based boards including:
- BTT SKR Pico
- Raspberry Pi Pico
- Adafruit Feather RP2040
- Other RP2040 boards

## Features

- **Flash Storage**: Wear-leveled key-value storage using `sequential-storage`
- **GPIO**: Digital input/output with interrupt support
- **UART**: Async serial communication
- **I2C**: Async I2C master (for displays, sensors)
- **SPI**: Async SPI master

## Usage

This HAL is typically used by `isochron-firmware` and selected via the build system:

```bash
make profile PROFILE=btt-pico
make build
```

## Pin Configuration

Pin assignments are defined in board configuration files:
- `configs/boards/btt-pico.toml` - BTT SKR Pico
- `configs/boards/pico.toml` - Raspberry Pi Pico

## Dependencies

- `embassy-rp` - Embassy HAL for RP2040
- `embassy-time` - Async timing primitives
- `sequential-storage` - Flash key-value storage
- `isochron-hal` - Shared trait definitions

## Flash Storage

The RP2040 implementation uses the last 256KB of flash for configuration storage:

```
Flash Layout (2MB total):
├── 0x10000000 - 0x101BFFFF: Firmware (1.75MB)
└── 0x101C0000 - 0x101FFFFF: Config storage (256KB)
```

Storage keys:
- `MachineConfig` - Machine configuration (cycles, temperatures)
- `CalibrationData` - Sensor calibration
- `RuntimeState` - Persistent runtime state
- `UserPreferences` - User settings

## License

GPL-3.0-or-later
