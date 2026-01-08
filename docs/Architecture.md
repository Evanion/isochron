# Isochron Architecture

This document describes the modular architecture of the Isochron watch cleaner firmware.

## Overview

Isochron uses a Klipper-inspired architecture with shared traits and chip-specific implementations. This allows supporting multiple controller boards and display types without code duplication.

```
isochron/
├── hal/                          # Hardware Abstraction Layers
│   ├── isochron-hal/            # Shared traits (chip-agnostic)
│   ├── isochron-hal-rp2040/     # RP2040 implementation
│   └── isochron-hal-stm32f0/    # STM32F0 implementation (WIP)
│
├── displays/                     # Display modules
│   ├── isochron-display/        # Display traits + shared code
│   └── isochron-display-fw/     # Display firmware (WIP)
│
├── isochron-core/               # Core logic (config, cycles, PID)
├── isochron-drivers/            # Device drivers (steppers, heaters)
├── isochron-protocol/           # UART protocol (controller <-> display)
├── isochron-firmware/           # Main controller firmware
│   ├── machine.toml             # Machine configuration (embedded at build)
│   └── examples/                # Example configurations
│
├── profiles/                     # Build profiles
│   ├── shipped/                 # Pre-defined profiles
│   └── user/                    # User-saved profiles
│
├── Kconfig                      # Build configuration schema
└── Makefile                     # Build system
```

## HAL Layer

The Hardware Abstraction Layer separates chip-specific code from application logic.

### Shared Traits (`isochron-hal`)

Defines traits that all chip-specific HALs implement:

```rust
// Flash storage
pub trait FlashStorage {
    async fn read(&mut self, key: StorageKey, buffer: &mut [u8]) -> Result<usize, FlashError>;
    async fn write(&mut self, key: StorageKey, data: &[u8]) -> Result<(), FlashError>;
    async fn exists(&mut self, key: StorageKey) -> bool;
    async fn erase_all(&mut self) -> Result<(), FlashError>;
}

// GPIO
pub trait OutputPin {
    fn set_high(&mut self);
    fn set_low(&mut self);
    fn toggle(&mut self);
}

pub trait InputPin {
    fn is_high(&self) -> bool;
    fn is_low(&self) -> bool;
}

// Communication
pub trait UartTx { ... }
pub trait UartRx { ... }
pub trait I2cBus { ... }
pub trait SpiBus { ... }
```

### Chip-Specific HALs

Each supported chip family has its own HAL crate:

- **`isochron-hal-rp2040`**: Raspberry Pi RP2040 (BTT Pico, Pi Pico, Feather RP2040)
- **`isochron-hal-stm32f0`**: STM32F0 series (V0 display MCU)

HALs are use-case agnostic - the same HAL can be used for controller boards OR display MCUs.

## Display Layer

### Display Traits (`isochron-display`)

```rust
pub trait DisplayBackend {
    fn clear(&mut self) -> Result<(), DisplayError>;
    fn draw_text(&mut self, row: u8, col: u8, text: &str) -> Result<(), DisplayError>;
    fn invert_region(&mut self, row: u8, start: u8, end: u8) -> Result<(), DisplayError>;
    fn flush(&mut self) -> Result<(), DisplayError>;
    fn dimensions(&self) -> (u8, u8);
}

pub enum NavigationEvent {
    ScrollUp,
    ScrollDown,
    Select,
    Back,
    LongSelect,
}
```

### Display Types

1. **External displays** (V0 Mini): Separate MCU, communicates via UART protocol
2. **Direct displays**: Connected directly to controller (I2C/SPI OLED)

## Build System

Uses kconfiglib (same as Klipper) for familiar configuration workflow.

### Quick Start

```bash
# Install prerequisites
pip install kconfiglib
cargo install elf2uf2-rs

# Configure
make menuconfig    # Interactive configuration
# OR
make profile PROFILE=btt-pico  # Load pre-defined profile

# Build
make build

# Flash
make flash
# OR copy out/isochron-firmware.uf2 to BOOTSEL drive
```

### Profiles

Profiles save complete build configurations for quick switching:

```bash
make list-profiles              # Show available
make profile PROFILE=btt-pico   # Load shipped profile
make save-profile PROFILE=my-setup  # Save current as user profile
```

## Configuration

Like Klipper's `printer.cfg`, Isochron uses a single `machine.toml` file that contains everything:
- Pin assignments for the board
- Motor configurations (with Klipper-style position limits)
- Heater settings
- Jar definitions
- Cleaning profiles and programs
- Display settings

Configuration is in `isochron-firmware/machine.toml` and embedded at build time.

### Example Configuration

See `isochron-firmware/examples/` for complete examples:
- `btt-pico.toml` - Full BTT SKR Pico example with all features
- `btt-pico-minimal.toml` - Minimal configuration (basket motor + heater)
- `custom-rp2040.toml` - Template for custom boards

### Stepper Configuration (Klipper-style)

```toml
[stepper.x]
step_pin = "gpio11"
dir_pin = "gpio10"
enable_pin = "!gpio12"        # Active low
rotation_distance = 200       # mm arc distance per motor rotation
gear_ratio = "20:16"
full_steps_per_rotation = 200
microsteps = 16
endstop_pin = "^gpio4"        # Pull-up enabled
position_min = 0              # mm (Klipper-style)
position_max = 800            # mm
position_endstop = 0          # mm at endstop
homing_speed = 30             # mm/s
```

## Adding New Hardware

### New RP2040 Board

1. Copy `isochron-firmware/examples/custom-rp2040.toml` to `machine.toml`
2. Update GPIO pin numbers for your board
3. Create profile with `make save-profile PROFILE=my-board`
4. Build with `make profile PROFILE=my-board && make build`

### New Chip Family

1. Create `hal/isochron-hal-<chip>/` implementing shared traits
2. Add to workspace (if embassy versions compatible) or build standalone
3. Update Kconfig with new board options

### New Display Type

1. Implement `DisplayBackend` trait for your display
2. Add display configuration section to `machine.toml`
3. Update Kconfig display options

## Motor Types

Isochron supports multiple motor types via the driver abstraction layer.

### Configuration

Motor type is selected in the machine configuration:

```toml
[machine]
motor_type = "stepper"  # or "dc", "ac"
```

### Driver Traits

The driver layer abstracts motor control via traits in `isochron-core::traits`:

```rust
// Base trait for all motors
pub trait MotorDriver {
    fn set_direction(&mut self, dir: Direction);
    fn enable(&mut self, enabled: bool);
    fn start(&mut self) -> Result<(), MotorError>;
    fn stop(&mut self);
    fn is_running(&self) -> bool;
}

// DC motors with PWM speed control
pub trait DcMotorDriver: MotorDriver {
    fn set_speed(&mut self, percent: u8);  // 0-100%
    fn get_actual_speed(&self) -> u8;
    fn update(&mut self) -> u8;  // Returns duty cycle
}

// AC motors with relay control
pub trait AcMotorDriver: MotorDriver {
    fn can_switch(&self) -> bool;  // Relay timing protection
    fn update(&mut self);
}
```

### Supported Motor Types

| Type | Control | Traits | Use Case |
|------|---------|--------|----------|
| Stepper | Step/dir interface | `StepperDriver` | Precise positioning |
| DC | PWM duty cycle | `DcMotorDriver` | Variable speed, budget builds |
| AC | On/off relay | `AcMotorDriver` | Fixed speed, industrial retrofit |

DC and AC motor support enables compact boards (Feather RP2040, XIAO) to run basic cleaners without external stepper drivers.

## Future Considerations

### ESP32 Support

Could be added with `isochron-hal-esp32` using Embassy's ESP32 support.
