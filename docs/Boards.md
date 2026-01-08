# Supported Boards

Isochron firmware is designed to run on RP2040-based boards. This page documents supported boards and their configurations.

## Board Selection

The firmware uses Cargo features for board selection:

```bash
# Build for SKR Pico (default)
cargo build --release

# Explicitly specify board
cargo build --release --features board-skr-pico
```

---

## BTT SKR Pico

The **BigTreeTech SKR Pico** is the primary supported board. It's a 3D printer controller based on the RP2040, repurposed for watch cleaning machine control.

### Specifications

| Feature | Specification |
|---------|---------------|
| MCU | RP2040 (dual-core Cortex-M0+ @ 133MHz) |
| Flash | 2MB (W25Q16) |
| RAM | 264KB SRAM |
| Stepper Drivers | 4x TMC2209 (UART mode) |
| Heater Outputs | 2x MOSFET (HE0, HB) |
| Thermistor Inputs | 2x ADC (TH0, TB) |
| Fan Outputs | 3x PWM |
| Input Voltage | 12-24V DC |

### Pinout

```
SKR Pico v1.0 Pin Assignments
═══════════════════════════════════════════════════════════════

STEPPER X (x motor)            STEPPER Y (lid motor)
├─ STEP:   GPIO11              ├─ STEP:   GPIO6
├─ DIR:    GPIO10              ├─ DIR:    GPIO5
├─ ENABLE: GPIO12 (active low) ├─ ENABLE: GPIO7 (active low)
└─ DIAG:   GPIO17              └─ DIAG:   GPIO3

STEPPER Z (z motor)            STEPPER E (basket motor)
├─ STEP:   GPIO19              ├─ STEP:   GPIO14
├─ DIR:    GPIO28              ├─ DIR:    GPIO13
├─ ENABLE: GPIO2 (active low)  ├─ ENABLE: GPIO15 (active low)
└─ DIAG:   GPIO25              └─ DIAG:   -

TMC2209 UART (Shared Bus)      DISPLAY UART
├─ TX:     GPIO8               ├─ TX:     GPIO0
└─ RX:     GPIO9               └─ RX:     GPIO1

HEATER OUTPUTS                 THERMISTOR INPUTS
├─ HE0:    GPIO23              ├─ TH0:    GPIO27 (ADC1)
└─ HB:     GPIO21              └─ TB:     GPIO26 (ADC0)

FAN OUTPUTS                    ENDSTOPS
├─ FAN0:   GPIO17              ├─ X:      GPIO4
├─ FAN1:   GPIO18              ├─ Y:      GPIO3
└─ FAN2:   GPIO20              └─ Z:      GPIO25

OTHER
├─ Neopixel: GPIO24
├─ Filament: GPIO16
├─ Probe:    GPIO22, GPIO29
└─ SWD:      SWCLK, SWDIO (debug header)
```

### Default Configuration

The default firmware config uses these assignments:

| Motor | Connector | Pins | TMC ADDR |
|-------|-----------|------|----------|
| basket | Stepper E | STEP=14, DIR=13, EN=15 | 3 |
| x (optional) | Stepper X | STEP=11, DIR=10, EN=12 | 0 |
| z (optional) | Stepper Z | STEP=19, DIR=28, EN=2 | 2 |
| lid (optional) | Stepper Y | STEP=6, DIR=5, EN=7 | 1 |

| Function | Hardware | Pins |
|----------|----------|------|
| TMC2209 | UART bus | TX=8, RX=9 |
| Dryer Heater | HE0 | OUT=23, THERM=27 |
| Display | UART0 | TX=0, RX=1 |

### Connector Reference

```
┌──────────────────────────────────────────────────────────────┐
│                        SKR Pico v1.0                         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  [PWR]  24V power input (VIN, GND)                          │
│                                                              │
│  [X]    Stepper X connector (x motor - jar selection)       │
│  [Y]    Stepper Y connector (lid motor)                     │
│  [Z]    Stepper Z connector (z motor - lift)                │
│  [E]    Stepper E connector (basket motor)                  │
│                                                              │
│  [HE0]  Heater 0 output (dryer heater) ← Use this          │
│  [HB]   Heated bed output (spare)                           │
│                                                              │
│  [TH0]  Thermistor 0 input (dryer temp) ← Use this         │
│  [TB]   Thermistor bed input (spare)                        │
│                                                              │
│  [FAN0-2] Fan outputs (PWM capable)                         │
│                                                              │
│  [UART] 4-pin UART header (for V0 Display) ← Use this      │
│         Pin 1: 5V                                           │
│         Pin 2: GND                                          │
│         Pin 3: TX (GPIO0)                                   │
│         Pin 4: RX (GPIO1)                                   │
│                                                              │
│  [SWD]  Debug header (SWCLK, SWDIO, GND)                   │
│                                                              │
│  [USB]  USB-C for programming/power                         │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Purchase

Available from:
- [BigTreeTech Official Store](https://biqu.equipment/products/btt-skr-pico-v1-0)
- Amazon
- AliExpress

---

## Future Boards

The following boards are planned for future support:

### Raspberry Pi Pico / Pico W

**Status:** Planned

Basic RP2040 board without integrated stepper drivers. Requires external driver boards.

| Feature | Notes |
|---------|-------|
| MCU | RP2040 |
| Drivers | External (A4988, TMC2209 modules) |
| Best For | Custom builds, prototyping |

### SKR Pico v2

**Status:** When Released

Updated version of SKR Pico with potential improvements.

### Custom RP2040 Boards

The firmware architecture supports custom boards through:

1. Create new board definition in `src/boards/`
2. Add feature flag in `Cargo.toml`
3. Define pin mappings and defaults

---

## Hardware Requirements

### Minimum Requirements

For a basic manual cleaning machine:

| Component | Purpose |
|-----------|---------|
| 1x Stepper motor | Basket rotation |
| 1x TMC2209 driver | Motor control |
| 1x Heater + thermistor | Drying chamber |
| 1x V0 Display | User interface |

### Full Automated Machine

For a fully automated cleaning machine:

| Component | Purpose |
|-----------|---------|
| 4x Stepper motors | basket, x, z, lid |
| 4x TMC2209 drivers | Motor control |
| 1-4x Heaters | Per-jar heating |
| 1x V0 Display | User interface |
| Endstops | Position feedback |

---

## Wiring Guidelines

### Power

- **Input voltage**: 12-24V DC
- **Current**: Size PSU for heater + motors (~200W typical)
- **Ground**: Common ground between PSU, board, and motors

### Stepper Motors

- Use shielded cables for long runs
- Keep motor cables away from signal cables
- Verify motor phase wiring (swap A/B pairs if direction wrong)

### TMC2209 UART

- Short traces preferred (on-board best)
- For external modules: Keep UART wires short (<10cm)
- Pull-up resistors may be needed for external modules

### Thermistors

- Use shielded twisted pair for thermistor wires
- Keep away from heater and motor wires
- Verify thermistor type matches config (NTC 100K typical)

### Display

- UART connection (TX, RX, GND, 5V)
- Cable length up to 1m typically works
- Use shielded cable for longer runs

---

## Board Comparison

| Feature | SKR Pico | Pico (planned) |
|---------|----------|----------------|
| Price | ~$25 | ~$4 |
| Drivers | 4x TMC2209 | External |
| Heaters | 2x MOSFET | External |
| Thermistors | 2x ADC | 3x ADC |
| Complexity | Plug & play | DIY wiring |
| Best For | Production | Prototyping |

---

## Creating a New Board Definition

To add support for a new board:

1. **Create board module** at `src/boards/your_board.rs`:

```rust
//! Your Board definition

pub mod pins {
    pub const STEP_X: u8 = 11;  // Adjust for your board
    pub const DIR_X: u8 = 10;
    // ... etc
}

pub fn default_config() -> MachineConfig {
    // Return default config for this board
}
```

2. **Add feature flag** in `Cargo.toml`:

```toml
[features]
board-skr-pico = []
board-your-board = []
default = ["board-skr-pico"]
```

3. **Update boards/mod.rs**:

```rust
#[cfg(feature = "board-your-board")]
pub mod your_board;

#[cfg(feature = "board-your-board")]
pub use your_board::*;
```

4. **Test thoroughly** before submitting PR.
