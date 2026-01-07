# Isochron - Implementation Plan

## Overview

**Isochron** is embedded Rust firmware for RP2040-based boards (SKR Pico, etc.) controlling watch cleaning machines. Named after the Greek "isochronous" (ἰσόχρονος) meaning "equal time" - reflecting both the precision timing of watch movements and the consistent cleaning cycles this firmware provides.

**Klipper-inspired architecture**: all hardware is defined in config, supporting various machine configurations. Uses Embassy async framework with binary (postcard) configuration.

## Machine Architecture

A watch cleaning machine has a **single basket** that moves through multiple jars (clean → rinse → dry):

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
│                            ↑                                 │
│                    lift motor (optional)                    │
│                            ↑                                 │
│                 position motor (optional)                   │
└─────────────────────────────────────────────────────────────┘
```

**Stepper motors (progressive complexity):**
- `spin` - rotates basket in solution (always required)
- `lift` - raises/lowers basket into jars (optional)
- `position` - moves basket between jars (optional)
- `lid` - opens/closes drying chamber (optional)

**Accessories (optional):**
- Per-jar heaters, ultrasonic modules, fans, pumps
- Neopixels for status/decoration
- Speaker for notifications

## Design Philosophy

Like Klipper, the firmware is **config-driven** and **portable**:
- All pins defined in config, not hardcoded
- Support for multiple motor/heater instances
- Modular component sections (stepper, heater, display)
- Users can adapt to their own hardware designs
- **Board-agnostic core** with board-specific HAL implementations
- **Driver abstraction** for different stepper drivers (TMC2209, TMC2130, A4988)

## Portability Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│  state machine, scheduler, profiles, safety, display         │
│  (board-agnostic, uses traits)                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Driver Traits                            │
│  StepperDriver, TemperatureSensor, HeaterOutput, Display     │
│  (abstract interface, multiple implementations)              │
└─────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  tmc2209        │ │  tmc2130        │ │  a4988          │
│  (UART)         │ │  (SPI)          │ │  (STEP/DIR only)│
└─────────────────┘ └─────────────────┘ └─────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Board HAL                               │
│  GPIO, UART, SPI, ADC, PIO, Timer                           │
│  (board-specific implementation)                            │
└─────────────────────────────────────────────────────────────┘
          │                   │                   │
          ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  rp2040         │ │  stm32f4        │ │  stm32g0        │
│  (SKR Pico)     │ │  (future)       │ │  (future)       │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

**Board selection:** Cargo features (e.g., `--features board-skr-pico`)
**Driver selection:** Config file (e.g., `[tmc2209 basket]` vs `[a4988 basket]`)

### Key Traits

```rust
// cleaner-core/src/traits/stepper.rs
pub trait StepperDriver {
    fn set_rpm(&mut self, rpm: u16);
    fn set_direction(&mut self, dir: Direction);
    fn enable(&mut self, enabled: bool);
    fn is_stalled(&self) -> bool;
}

// cleaner-core/src/traits/heater.rs
pub trait TemperatureSensor {
    fn read_celsius(&self) -> Result<i16, SensorError>;
}

pub trait HeaterOutput {
    fn set_on(&mut self, on: bool);
}
```

These traits allow the application layer to work with any driver implementation.

## Project Structure

```
chronohub/cleaner/
├── Cargo.toml                    # Workspace root
├── brief.md                      # Specification (existing)
│
├── cleaner-core/                 # Board-agnostic application logic
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── traits/               # Hardware abstraction traits
│       │   ├── mod.rs
│       │   ├── stepper.rs        # StepperDriver trait
│       │   ├── heater.rs         # HeaterOutput, TemperatureSensor traits
│       │   └── display.rs        # Display trait
│       ├── state/
│       │   ├── mod.rs
│       │   ├── machine.rs        # State enum, transitions
│       │   └── events.rs         # Event enum
│       ├── scheduler/
│       │   ├── mod.rs
│       │   └── segment.rs        # Profile → segments
│       ├── motion/
│       │   ├── mod.rs
│       │   └── planner.rs        # Acceleration profiles (math only)
│       ├── safety/
│       │   ├── mod.rs
│       │   └── monitor.rs        # Fault detection logic
│       └── config/
│           ├── mod.rs
│           └── types.rs          # Config structs (board-agnostic)
│
├── cleaner-drivers/              # Hardware driver implementations
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── stepper/
│       │   ├── mod.rs
│       │   ├── tmc2209.rs        # TMC2209 (UART)
│       │   ├── tmc2130.rs        # TMC2130 (SPI) - future
│       │   └── a4988.rs          # A4988 (STEP/DIR only) - future
│       ├── heater/
│       │   ├── mod.rs
│       │   ├── bang_bang.rs      # Simple on/off control
│       │   └── pid.rs            # PID control - future
│       ├── sensor/
│       │   ├── mod.rs
│       │   └── ntc100k.rs        # NTC thermistor
│       └── accessory/
│           ├── mod.rs
│           ├── ultrasonic.rs     # Ultrasonic cleaner module
│           ├── neopixel.rs       # WS2812 LED strip
│           ├── fan.rs            # PWM/on-off fan
│           └── speaker.rs        # Buzzer/speaker
│
├── cleaner-hal-rp2040/           # RP2040-specific HAL
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── gpio.rs               # GPIO allocation
│       ├── uart.rs               # UART peripheral allocation
│       ├── adc.rs                # ADC channel allocation
│       ├── pio.rs                # PIO step generator
│       └── flash.rs              # Flash storage
│
├── cleaner-firmware/             # Main firmware binary
│   ├── Cargo.toml                # Features: board-skr-pico, board-xxx
│   ├── memory.x                  # Linker script (board-specific)
│   ├── .cargo/config.toml        # Target config
│   └── src/
│       ├── main.rs               # Entry, config parse, instantiation
│       ├── boards/
│       │   ├── mod.rs
│       │   └── skr_pico.rs       # SKR Pico board definition
│       ├── config/
│       │   ├── mod.rs
│       │   ├── loader.rs         # Flash read, TOML parse
│       │   ├── parser.rs         # Section parsing
│       │   └── pin.rs            # Pin string parsing
│       ├── components/           # Component instantiation
│       │   ├── mod.rs
│       │   ├── stepper.rs        # Stepper from config
│       │   ├── heater.rs         # Heater from config
│       │   └── display.rs        # Display from config
│       └── display/
│           ├── mod.rs
│           ├── protocol.rs       # Frame encode/decode
│           ├── link.rs           # Heartbeat management
│           └── renderer.rs       # Screen building
│
├── cleaner-protocol/             # Shared protocol types
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── frame.rs              # Wire format
│       ├── messages.rs           # Message types
│       └── events.rs             # Input events
│
└── cleaner-display/              # V0 Display firmware (future)
```

**Crate responsibilities:**
- `cleaner-core`: Board-agnostic logic (traits, state machine, scheduler)
- `cleaner-drivers`: Stepper driver implementations (tmc2209, etc.)
- `cleaner-hal-rp2040`: RP2040-specific HAL (gpio, uart, pio, flash)
- `cleaner-firmware`: Binary with board-specific instantiation
- `cleaner-protocol`: Shared display protocol types

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `embassy-executor` | Async task executor |
| `embassy-rp` | RP2040 HAL |
| `embassy-time` | Timers |
| `embassy-sync` | Channels, signals |
| `embedded-alloc` | Heap allocator (for TOML) |
| `toml` (0.9+) | Runtime TOML parsing (no_std + alloc) |
| `sequential-storage` | Flash key-value with wear leveling |
| `postcard` | Binary serialization for flash |
| `heapless` | Fixed-capacity collections |
| `defmt` + `defmt-rtt` | Debug logging |
| `panic-probe` | Panic handler |

## Config Format (Klipper-inspired)

```toml
# machine.toml - Hardware configuration

[mcu]
# RP2040 board, pins use gpio numbers

# === STEPPERS (Klipper-style) ===

# Basket spin motor (rotates parts in solution) - always required
[stepper spin]
step_pin = "gpio11"
dir_pin = "gpio10"
enable_pin = "!gpio12"              # ! = active low
rotation_distance = 360             # degrees per motor rotation
gear_ratio = "3:1"                  # 3:1 belt reduction
full_steps_per_rotation = 200       # 1.8° motor (default)
microsteps = 16

[tmc2209 spin]
uart_pin = "gpio9"
tx_pin = "gpio8"
uart_address = 0
run_current = 0.8
stealthchop = true

# Lift motor (raises/lowers basket) - optional
# [stepper lift]
# step_pin = "gpio6"
# dir_pin = "gpio5"
# enable_pin = "!gpio7"
# rotation_distance = 8             # mm per leadscrew rotation
# full_steps_per_rotation = 200
# microsteps = 16
# endstop_pin = "^gpio4"            # ^ = pull-up, physical endstop
# # OR use stallguard:
# # endstop_pin = "tmc2209_lift:virtual_endstop"
# position_endstop = 0              # mm at endstop
# position_max = 150                # mm max travel
# homing_speed = 10                 # mm/s

# [tmc2209 lift]
# uart_address = 1
# diag_pin = "gpio3"                # For stallguard
# driver_sgthrs = 80                # Stallguard threshold

# Position/rotation motor (rotates tower between jars) - optional
# [stepper tower]
# step_pin = "gpio19"
# dir_pin = "gpio28"
# enable_pin = "!gpio2"
# rotation_distance = 360           # degrees per motor rotation
# gear_ratio = "5:1"                # Tower gearbox
# microsteps = 16
# endstop_pin = "^gpio16"           # Home switch
# # OR: endstop_pin = "tmc2209_tower:virtual_endstop"
# position_endstop = 0              # degrees at home
# position_max = 360                # degrees (full rotation)
# homing_speed = 30                 # deg/s

# [tmc2209 tower]
# uart_address = 2
# diag_pin = "gpio17"
# driver_sgthrs = 60

# === HEATERS ===

[heater dryer]
heater_pin = "gpio23"
sensor_pin = "gpio27"
sensor_type = "ntc100k"
control = "bang_bang"
max_temp = 55
hysteresis = 2

# === JARS (define physical jar positions) ===
# Positions are coordinates from endstop after homing
# - For tower rotation: degrees from home
# - For linear position: mm from home
# - For lift: mm down from top

[jar clean]
tower_pos = 0                       # degrees from home (first jar at home)
lift_pos = 120                      # mm down (lowered into jar)
# heater = "jar_clean"              # Optional per-jar heater
# ultrasonic = "us_clean"           # Optional ultrasonic module

[jar rinse1]
tower_pos = 90                      # 90 degrees rotation
lift_pos = 120

[jar rinse2]
tower_pos = 180                     # 180 degrees rotation
lift_pos = 120

[jar dry]
tower_pos = 270                     # 270 degrees rotation
lift_pos = 100                      # Shallower for drying chamber
heater = "dryer"                    # Drying chamber heater

# === ACCESSORIES ===

# [ultrasonic us_clean]
# trigger_pin = "gpio16"
# power = 100

# [neopixel status]
# pin = "gpio24"
# count = 8

# [fan exhaust]
# pin = "gpio17"
# pwm = true

# [speaker notify]
# pin = "gpio22"

# === DISPLAY ===

[display]
type = "v0_display"
uart_tx_pin = "gpio0"
uart_rx_pin = "gpio1"
baud = 115200
```

```toml
# profiles.toml - Profile definitions (per-jar behavior)

[profile.clean]
label = "Clean"
rpm = 120
time_s = 180
direction = "alternate"
iterations = 3

# Spin-off phase: lift basket slightly and spin to shed excess solution
# Prevents contamination/dripping when moving to next jar
[profile.clean.spinoff]
lift_mm = 20                        # mm to lift above jar (automated only)
rpm = 150                           # fast spin to shed solution
time_s = 10                         # duration of spin-off

[profile.rinse]
label = "Rinse"
rpm = 100
time_s = 120
direction = "cw"

[profile.rinse.spinoff]
lift_mm = 20
rpm = 150
time_s = 8

[profile.dry]
label = "Dry"
rpm = 60
time_s = 600
direction = "alternate"
iterations = 6
temperature_c = 45                  # Heater target (uses jar's heater)
# No spinoff for dry - it's the final step
```

```toml
# programs.toml - Program definitions (sequences of jar/profile pairs)

[program.full_clean]
label = "Full Clean"
steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
    { jar = "rinse2", profile = "rinse" },
    { jar = "dry",    profile = "dry" },
]

[program.quick_clean]
label = "Quick Clean"
steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
]

[program.dry_only]
label = "Dry Only"
steps = [
    { jar = "dry", profile = "dry" },
]

# For manual machines (no lift/position motors), user is prompted to move basket
# For automated machines, firmware handles lift/position between steps
```

## Runtime Config Approach

1. Store raw TOML text in flash partition (last 64KB)
2. At boot: read TOML bytes from flash → parse with `toml` crate
3. Instantiate hardware components based on config sections
4. Validate profiles against available hardware
5. If invalid: fall back to built-in defaults, show error
6. Session edits held in RAM, optionally saved back to flash

Requires ~10-20KB extra binary size for TOML parser + heap.

## Task Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  main_task: State Machine + Coordinator                     │
│  - Parses config, instantiates components                   │
│  - Receives events from EVENT_CHANNEL                       │
│  - Routes commands to appropriate component instances       │
│  - Owns profile context and current state                   │
└─────────────────────────────────────────────────────────────┘
      ▲ Events              │ Commands (by name)
      │                     ▼
┌─────┴─────┐  ┌───────────────────────────────┐  ┌──────────────┐
│ display   │  │ motor_tasks[]                 │  │ display_tx   │
│ _rx_task  │  │ - One task per [stepper name] │  │ _task        │
└───────────┘  │ - PIO step generation         │  └──────────────┘
               └───────────────────────────────┘

┌───────────┐  ┌───────────────────────────────┐  ┌──────────────┐
│ scheduler │  │ heater_tasks[]                │  │ safety_task  │
│ _task     │  │ - One task per [heater name]  │  │ (monitors    │
│           │  │ - Bang-bang control           │  │  all devices)│
└───────────┘  └───────────────────────────────┘  └──────────────┘
```

**Multi-instance task spawning:**
- Tasks are spawned dynamically based on config
- Each motor/heater gets its own task
- Commands include target name: `MotorCmd { name: "clean", rpm: 120 }`
- Safety task monitors all heater instances

**Inter-task communication:**
- `EVENT_CHANNEL`: All → State Machine (encoder, scheduler, safety)
- `MOTOR_CMDS[name]`: State Machine → specific Motor task
- `HEATER_CMDS[name]`: State Machine → specific Heater task
- `DISPLAY_CHANNEL`: State Machine → Display TX
- `TEMPS`: All heaters → Safety task (temperature readings)

## State Machine

States:
- `Boot` - initialization, config loading
- `Idle` - program list displayed, awaiting selection
- `ProgramSelected` - program details shown, awaiting start
- `AwaitingJar` - waiting for user to move basket (manual machines)
- `Running` - profile executing (spin motor active in jar)
- `SpinOff` - basket lifted, spinning to shed excess solution
- `AwaitingSpinOff` - waiting for user to lift basket for spin-off (manual)
- `Paused` - profile paused by user
- `StepComplete` - jar step done, moving to next (or prompting user)
- `ProgramComplete` - all steps done
- `Error(kind)` - fault detected

**Program execution flow (automated machine):**
```
Idle → ProgramSelected → Running → SpinOff → StepComplete
                            ↑                     │
                            └─────────────────────┘
                                  (next jar)
```

**Program execution flow (manual machine):**
```
Idle → ProgramSelected → AwaitingJar → Running → AwaitingSpinOff → SpinOff → StepComplete
                              ↑                                                    │
                              └────────────────────────────────────────────────────┘
                                                  (next jar)
```

**Spin-off phase details:**
- **Automated machines**: After profile completes, firmware lifts basket by `spinoff.lift_mm`, spins at `spinoff.rpm` for `spinoff.time_s`, then proceeds to next jar
- **Manual machines**: Display prompts "Lift basket, press to continue", user lifts manually, spin-off runs, then prompts for next jar
- **Optional**: If no `[profile.*.spinoff]` section, skip spin-off phase entirely

For **manual machines** (no lift/position): User prompted to move basket between jars
For **automated machines**: Firmware controls lift/position steppers between steps

Key invariants from brief:
- Heater only ON in Running state
- Motor direction never reverses at speed
- Any error → immediate transition to Error
- Link failure stops motor (controlled decel)

## Implementation Phases

### Phase 1: Foundation
1. Create workspace with all crates (core, drivers, hal-rp2040, firmware, protocol)
2. Set up RP2040 target, memory.x, cargo config, heap allocator
3. Verify basic Embassy blinky works on SKR Pico
4. Implement protocol frame encoding/decoding with tests

### Phase 2: Core Traits & State Machine
5. Define StepperDriver trait (set_rpm, set_direction, enable, stall_detected)
6. Define HeaterOutput, TemperatureSensor traits
7. Define State and Event enums with transition logic
8. Implement motion planner (acceleration math, no hardware)

### Phase 3: Config System (Klipper-style)
9. Flash storage driver with sequential-storage
10. TOML loading and parsing at boot
11. Pin string parser ("!gpio12" → Pin with invert flag)
12. Section parsers: [stepper name], [tmc2209 name], [heater name]
13. Config validation and default fallback

### Phase 4: RP2040 HAL
14. GPIO allocator (track used pins, prevent conflicts)
15. UART allocator (manage UART0/UART1 instances)
16. ADC allocator (manage ADC channels)
17. PIO step generator (shared program, per-instance SM)

### Phase 5: Driver Implementations
18. TMC2209 driver (implements StepperDriver trait via UART)
19. A4988 driver stub (STEP/DIR only, for future)
20. NTC100K thermistor (implements TemperatureSensor)
21. GPIO heater output (implements HeaterOutput)

### Phase 6: Component Instantiation
22. Stepper component factory (config → driver + motion)
23. Heater component factory (config → sensor + output)
24. Display component (single instance)
25. Dynamic task spawning based on config

### Phase 7: Display Communication
26. UART RX task: frame parsing, event extraction
27. UART TX task: frame building, command sending
28. Heartbeat management (link failure detection)
29. Screen renderer (menu, profile detail, running, error)

### Phase 8: Scheduler & Profiles
30. Profile parsing with stepper/heater references and optional spinoff section
31. Scheduler: profile → segment expansion (including spin-off segments)
32. Multi-motor profile support
33. Spin-off phase execution (lift + spin for automated, prompt + spin for manual)
34. Pause/resume with motor state preservation

### Phase 9: Safety & Integration
35. Safety monitor: all instances (temps, stall, link)
36. Error state handling
37. Full workflow testing
38. Edge cases and hardening

## Critical Files

| File | Responsibility |
|------|----------------|
| `cleaner-core/src/traits/stepper.rs` | StepperDriver trait (board-agnostic) |
| `cleaner-core/src/traits/heater.rs` | HeaterOutput, TemperatureSensor traits |
| `cleaner-core/src/state/machine.rs` | State enum, transition table |
| `cleaner-core/src/motion/planner.rs` | Acceleration math (no hardware) |
| `cleaner-drivers/src/tmc2209.rs` | TMC2209 implementation |
| `cleaner-hal-rp2040/src/pio.rs` | PIO step generator |
| `cleaner-hal-rp2040/src/gpio.rs` | GPIO allocation |
| `cleaner-firmware/src/main.rs` | Entry, config parse, instantiation |
| `cleaner-firmware/src/config/parser.rs` | Klipper-style section parsing |
| `cleaner-firmware/src/display/link.rs` | Heartbeat, link failure |
| `cleaner-protocol/src/frame.rs` | Wire format (shared) |

## Default Config: SKR Pico

The firmware ships with a default `machine.toml` for SKR Pico. Users can modify for other RP2040 boards.

**Reference pin assignments (Stepper X slot):**

| Function | GPIO | Notes |
|----------|------|-------|
| STEP | GPIO11 | Stepper X step |
| DIR | GPIO10 | Stepper X direction |
| ENABLE | GPIO12 | Stepper X enable (active low) |
| TMC UART TX | GPIO9 | Shared TMC bus |
| TMC UART RX | GPIO8 | Shared TMC bus, address 0 |
| Heater | GPIO23 | HE0 (hotend heater output) |
| Thermistor | GPIO27 | TH0 (hotend thermistor, ADC) |
| Display UART TX | GPIO0 | UART0 TX (dedicated UART connector) |
| Display UART RX | GPIO1 | UART0 RX (dedicated UART connector) |

**UART allocation:**
- UART0 (GPIO0/1): V0 Display communication (use board's UART connector)
- UART1 (GPIO8/9): TMC2209 driver bus (shared, addressed)

**Physical connector:** The SKR Pico has a 4-pin UART header: 5V, GND, GPIO0, GPIO1. Wire V0 Display's 3-pin GPIO header here.

**Alternative heater/thermistor (bed outputs):**
- Heater: GPIO21 (HB)
- Thermistor: GPIO26 (TB)

**Available for future use:**
- GPIO16 (filament sensor)
- GPIO17, GPIO18, GPIO20 (fan outputs)
- GPIO24 (neopixel)
- GPIO22, GPIO29 (BLTouch)

## Open Questions Resolved

- Config format: Runtime TOML with `toml` 0.9 (no_std + alloc)
- Display: V0 Display via UART, custom protocol
- Encoder: Handled by V0 Display, not SKR Pico
- Profile edits: Session-only (RAM), not persisted
- Link failure: Motor stops, requires power cycle

## Notes for Implementation

- Use `defmt` everywhere for debugging
- Keep patterns simple (user is new to embedded Rust)
- Comment non-obvious embedded idioms
- Test protocol crate on host with `cargo test`
- PIO programs need careful timing verification at boundary RPMs

## References

- [SKR Pico Klipper config](https://github.com/bigtreetech/SKR-Pico/blob/master/Klipper/SKR%20Pico%20klipper.cfg)
- [SKR Pico Pinout](https://github.com/bigtreetech/SKR-Pico/blob/master/Klipper/Images/pinout.png)
- [RP2040 UART pin mapping](https://piers.rocks/2023/09/08/pico-uart-pin-allocation.html)
- [toml v0.9 announcement (no_std support)](https://epage.github.io/blog/2025/07/toml-09/)
- [Voron V0 Display](https://github.com/VoronDesign/Voron-Hardware/tree/master/V0_Display)
