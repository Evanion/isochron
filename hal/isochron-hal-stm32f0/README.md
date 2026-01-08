# isochron-hal-stm32f0

STM32F0 implementation of the Isochron HAL traits.

This crate provides hardware abstraction for STM32F0-based boards, primarily the V0 Mini display module (STM32F042).

## Supported Chips

- STM32F042F6 (V0 Mini display)
- Other STM32F0 variants (with configuration)

## Features

- **GPIO**: Digital input/output
- **UART**: Async serial for protocol communication
- **I2C**: Async I2C master for OLED display
- **Flash**: Optional configuration storage

## Pin Configuration (V0 Mini)

| Function | Pin | Notes |
|----------|-----|-------|
| UART TX | PA2 | To controller |
| UART RX | PA15 | From controller |
| I2C SCL | PB6 | OLED display |
| I2C SDA | PB7 | OLED display |
| Encoder A | PA3 | Quadrature input |
| Encoder B | PA4 | Quadrature input |
| Encoder BTN | PA1 | Button input |

## Memory Constraints

The STM32F042F6 has limited resources:
- 32KB Flash
- 6KB RAM

The implementation is optimized for minimal memory usage.

## License

GPL-3.0-or-later
