# Isochron Firmware Architecture

## Overview

Isochron is embedded Rust firmware for RP2040-based watch cleaning machines. The architecture follows a **Klipper-inspired, config-driven design** where all hardware is defined in configuration, enabling support for various machine configurations.

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

A watch cleaning machine has a **single basket** that moves through multiple jars (clean → rinse → dry). The firmware supports both manual machines (user moves basket) and automated machines (firmware controls lift/position motors).

---

## Crate Structure

The project is organized as a Cargo workspace with distinct responsibilities:

```
chronohub/cleaner/
├── isochron-core/          # Board-agnostic application logic
├── isochron-drivers/       # Hardware driver implementations
├── isochron-hal-rp2040/    # RP2040-specific HAL
├── isochron-protocol/      # Display communication protocol
└── isochron-firmware/      # Main firmware binary
```

### isochron-core

**Purpose:** Board-agnostic logic (can be tested on host)

```
isochron-core/src/
├── traits/       # Hardware abstraction traits (StepperDriver, HeaterOutput)
├── state/        # State machine (State enum, Event enum, transitions)
├── scheduler/    # Profile → segment expansion, timing
├── motion/       # Acceleration math (ramp profiles, no hardware)
├── safety/       # Fault detection logic
└── config/       # Configuration type definitions
```

Key traits:
- `StepperDriver` - set_rpm, set_direction, enable, is_stalled
- `HeaterOutput` - set_on
- `TemperatureSensor` - read_celsius

### isochron-drivers

**Purpose:** Concrete driver implementations

```
isochron-drivers/src/
├── stepper/
│   ├── tmc2209.rs    # TMC2209 UART driver (StallGuard, stealthChop)
│   ├── tmc2130.rs    # TMC2130 SPI driver (future)
│   └── a4988.rs      # A4988 STEP/DIR only (future)
├── heater/
│   ├── bang_bang.rs  # Simple on/off control
│   └── pid.rs        # PID control (future)
├── sensor/
│   └── ntc100k.rs    # NTC thermistor ADC conversion
└── accessory/
    ├── ultrasonic.rs # Ultrasonic cleaner module
    ├── neopixel.rs   # WS2812 LED strip
    ├── fan.rs        # PWM/on-off fan
    └── speaker.rs    # Buzzer/speaker
```

### isochron-hal-rp2040

**Purpose:** RP2040-specific hardware abstraction

```
isochron-hal-rp2040/src/
├── gpio.rs       # GPIO allocation, conflict prevention
├── uart.rs       # UART peripheral management
├── adc.rs        # ADC channel management
├── pio.rs        # PIO step pulse generator
├── stepper.rs    # PioStepper implementation
└── flash.rs      # Flash storage driver
```

The PIO step generator uses RP2040's programmable I/O to generate precise step pulses without CPU involvement.

### isochron-protocol

**Purpose:** UART protocol between SKR Pico and V0 Display

```
isochron-protocol/src/
├── frame.rs      # Wire format (START, LENGTH, TYPE, PAYLOAD, CHECKSUM)
├── messages.rs   # Message types (DisplayCommand, PicoMessage)
└── events.rs     # Input events (encoder, button)
```

Frame format:
```
┌───────┬────────┬──────┬─────────────┬──────────┐
│ START │ LENGTH │ TYPE │ PAYLOAD     │ CHECKSUM │
│ 0xAA  │ 1B     │ 1B   │ 0–250B      │ 1B       │
└───────┴────────┴──────┴─────────────┴──────────┘
```

### isochron-firmware

**Purpose:** Main binary, board-specific instantiation

```
isochron-firmware/src/
├── main.rs           # Entry point, peripheral init, task spawning
├── boards/
│   └── skr_pico.rs   # SKR Pico board definition
├── channels.rs       # Inter-task communication (signals, channels)
├── controller/       # Controller logic (wraps state machine)
├── config/           # Config loading from flash
├── display/          # Screen rendering
└── tasks/
    ├── controller.rs # Main coordination task
    ├── stepper.rs    # Motor control task
    ├── heater.rs     # Temperature control task
    ├── display_rx.rs # UART receive task
    ├── display_tx.rs # UART transmit task
    ├── tmc.rs        # TMC2209 initialization
    ├── stall_monitor.rs # DIAG pin monitoring
    └── tick.rs       # Periodic tick generation
```

---

## Task Architecture

Embassy async tasks communicate via signals and channels:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        controller_task                               │
│  - Owns state machine and scheduler                                  │
│  - Receives: INPUT_CHANNEL, TICK_SIGNAL, TEMP_READING, MOTOR_STALL  │
│  - Sends: MOTOR_CMD, HEATER_CMD, SCREEN_UPDATE                      │
└─────────────────────────────────────────────────────────────────────┘
          ▲                    │                    ▲
          │                    │                    │
    ┌─────┴─────┐        ┌─────┴─────┐        ┌─────┴─────┐
    │INPUT_CHANNEL│       │MOTOR_CMD │        │TEMP_READING│
    └─────┬─────┘        └─────┬─────┘        └─────┬─────┘
          │                    │                    │
          │                    ▼                    │
┌─────────┴────────┐  ┌────────────────┐  ┌────────┴───────┐
│  display_rx_task │  │  stepper_task  │  │  heater_task   │
│  Parses frames,  │  │  PIO control,  │  │  ADC reading,  │
│  extracts events │  │  RPM, direction│  │  bang-bang PWM │
└──────────────────┘  └────────────────┘  └────────────────┘
                                                   │
┌──────────────────┐  ┌────────────────┐          │
│  display_tx_task │  │ stall_monitor  │          │
│  Sends screens,  │  │ _task          │          │
│  heartbeats      │  │ GPIO DIAG pin  │──► MOTOR_STALL
└──────────────────┘  └────────────────┘

┌──────────────────┐  ┌────────────────┐
│    tick_task     │  │  tmc_init_task │
│  100ms periodic  │  │  One-shot TMC  │
│  TICK_SIGNAL     │  │  configuration │
└──────────────────┘  └────────────────┘
```

### Communication Primitives

| Name | Type | Direction | Purpose |
|------|------|-----------|---------|
| `INPUT_CHANNEL` | Channel<8> | display_rx → controller | Encoder/button events |
| `EVENT_CHANNEL` | Channel<8> | controller → debug | State events (logging) |
| `SCREEN_UPDATE` | Signal | controller → display_tx | Trigger screen send |
| `HEARTBEAT_RECEIVED` | Signal | display_rx → controller | Link alive check |
| `MOTOR_CMD` | Signal | controller → stepper | RPM, direction, enable |
| `HEATER_CMD` | Signal | controller → heater | On/off, target temp |
| `TEMP_READING` | Signal | heater → controller | Current temp or fault |
| `MOTOR_STALL` | Signal | stall_monitor → controller | Stall detection |
| `TICK_SIGNAL` | Signal | tick → controller | Periodic 100ms tick |

### Task Responsibilities

| Task | Responsibility |
|------|----------------|
| `controller_task` | State machine, scheduler, safety coordination |
| `stepper_task` | PIO motor control, acceleration ramps |
| `heater_task` | ADC temperature reading, bang-bang control |
| `display_rx_task` | UART frame parsing, input extraction |
| `display_tx_task` | Screen building, frame transmission |
| `tmc_init_task` | One-shot TMC2209 register configuration |
| `stall_monitor_task` | GPIO-based DIAG pin monitoring |
| `tick_task` | 100ms periodic tick generation |

---

## State Machine

The state machine in `isochron-core/src/state/machine.rs` defines all possible states and transitions:

```
                    ┌─────────────────────────────────────────────┐
                    │                                             │
                    ▼                                             │
┌──────┐  BootComplete  ┌──────┐  SelectProgram  ┌─────────────┐ │
│ Boot │───────────────►│ Idle │───────────────►│ ProgramSel  │ │
└──────┘                └──────┘◄────Back────────└─────────────┘ │
   │                       ▲                           │         │
   │                       │                      Start│         │
   │                       │                           ▼         │
   │                  Abort│                    ┌─────────────┐  │
   │                       │  ┌─────Pause──────│   Running   │  │
   │                       │  │                └──────┬──────┘  │
   │                       │  ▼                       │         │
   │                  ┌──────────┐                    │         │
   │                  │  Paused  │──Resume───────────►│         │
   │                  └──────────┘                    │         │
   │                       │                   Profile│Finished │
   │                       │                          ▼         │
   │                       │                   ┌─────────────┐  │
   │                       │                   │ StepComplete│  │
   │                       │                   └──────┬──────┘  │
   │                       │                          │         │
   │                       │           ┌──NextStep────┘         │
   │                       │           │                        │
   │                       │           │  ProgramFinished       │
   │                       │           │         │              │
   │                       │           ▼         ▼              │
   │                       │    ┌─────────────────────┐         │
   │                       └────│  ProgramComplete    │─────────┘
   │                            └─────────────────────┘
   │
   │  ErrorDetected (from ANY state)
   ▼
┌─────────┐
│  Error  │ (terminal - requires power cycle)
└─────────┘
```

### States

| State | Motor | Heater | Description |
|-------|-------|--------|-------------|
| `Boot` | Off | Off | Initialization, config loading |
| `Idle` | Off | Off | Program list displayed |
| `ProgramSelected` | Off | Off | Program details shown |
| `AwaitingJar` | Off | Off | Manual: waiting for basket move |
| `Running` | **On** | **On*** | Profile executing |
| `AwaitingSpinOff` | Off | Off | Manual: waiting for basket lift |
| `SpinOff` | **On** | Off | Spinning to shed solution |
| `Paused` | Off | Off | User paused execution |
| `StepComplete` | Off | Off | Jar step done, transitioning |
| `ProgramComplete` | Off | Off | All steps finished |
| `Error` | Off | Off | Fault detected, outputs disabled |

*Heater only if profile has temperature target

### Key Invariants

1. **Heater only ON in Running state** - Never during SpinOff (basket out of solution)
2. **Motor direction never reverses at speed** - Must decelerate to 0 first
3. **Any error → immediate Error state** - All outputs disabled
4. **Link failure stops motor** - Controlled deceleration, then fault

---

## Safety System

The safety system monitors multiple fault conditions:

```
┌─────────────────────────────────────────────────────────────┐
│                    Safety Monitor                            │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────┐    ┌───────────────┐    ┌─────────────┐ │
│  │ Temperature   │    │ Motor Stall   │    │ Link Lost   │ │
│  │ Monitor       │    │ Monitor       │    │ Monitor     │ │
│  │               │    │               │    │             │ │
│  │ • Over-temp   │    │ • DIAG pin    │    │ • Heartbeat │ │
│  │ • Sensor fault│    │ • StallGuard  │    │ • Timeout   │ │
│  └───────┬───────┘    └───────┬───────┘    └──────┬──────┘ │
│          │                    │                   │        │
│          └────────────────────┼───────────────────┘        │
│                               ▼                            │
│                    ┌─────────────────┐                     │
│                    │ ErrorDetected() │                     │
│                    │ → Error state   │                     │
│                    └─────────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

### Monitored Conditions

| Condition | Source | Detection | Action |
|-----------|--------|-----------|--------|
| **Over-temperature** | heater_task | ADC > max_temp | ErrorKind::OverTemperature |
| **Sensor fault** | heater_task | ADC open/short | ErrorKind::ThermistorFault |
| **Motor stall** | stall_monitor_task | DIAG pin high | ErrorKind::MotorStall |
| **Link lost** | controller_task | No heartbeat 3s | ErrorKind::LinkLost |

### Data Flow

```
heater_task:
  ├─► TEMP_READING.signal(Some(temp_x10))  // Normal: temp in 0.1°C
  └─► TEMP_READING.signal(None)            // Fault: sensor error

stall_monitor_task:
  └─► MOTOR_STALL.signal(true)             // DIAG pin asserted

display_rx_task:
  └─► HEARTBEAT_RECEIVED.signal(())        // Ping received

controller_task:
  ├─ TEMP_READING.try_take() → update_temperature()
  ├─ MOTOR_STALL.try_take() → update_motor_stall()
  └─ HEARTBEAT_RECEIVED.signaled() → heartbeat_received()
```

### Temperature Monitoring

The heater task reads the thermistor ADC and converts to temperature:

```rust
// NTC 100K thermistor with 4.7K pullup
// ADC range 0-4095 maps to resistance range
// Steinhart-Hart equation converts resistance to temperature

if temp_c > config.max_temp_c {
    // Over-temperature fault
    TEMP_READING.signal(None);
} else {
    // Normal reading (0.1°C resolution)
    TEMP_READING.signal(Some(temp_c * 10));
}
```

### Stall Detection

TMC2209 StallGuard monitors motor load via DIAG pin:

```rust
// stall_monitor_task polls GPIO17 (DIAG pin)
// Debounce: 3 consecutive readings at 20ms intervals

if diag_pin.is_high() && debounce_count >= 3 {
    MOTOR_STALL.signal(true);
}
```

StallGuard threshold is configured via UART during TMC initialization:
- `driver_sgthrs = 80` - Higher = more sensitive
- Works best at moderate speeds (not very low RPM)

---

## Pin Assignments (SKR Pico)

See `docs/Boards.md` for complete pinout documentation.

### Motor Connectors

| Motor | Connector | STEP | DIR | EN | TMC ADDR |
|-------|-----------|------|-----|-----|----------|
| basket | E | GPIO14 | GPIO13 | GPIO15 | 3 |
| x | X | GPIO11 | GPIO10 | GPIO12 | 0 |
| z | Z | GPIO19 | GPIO28 | GPIO2 | 2 |
| lid | Y | GPIO6 | GPIO5 | GPIO7 | 1 |

### Peripherals

| Function | GPIO | Notes |
|----------|------|-------|
| TMC TX | GPIO8 | TMC2209 shared UART |
| TMC RX | GPIO9 | TMC2209 shared UART |
| Display TX | GPIO0 | UART0 to V0 Display |
| Display RX | GPIO1 | UART0 from V0 Display |
| Heater | GPIO23 | HE0 output (dryer) |
| Thermistor | GPIO27 | TH0 ADC input (dryer) |

---

## Configuration

The firmware uses a Klipper-inspired TOML configuration. At boot:

1. Read TOML from flash (last 64KB partition)
2. Parse hardware sections: `[stepper]`, `[tmc2209]`, `[heater]`
3. Instantiate drivers based on config
4. Validate profiles reference valid hardware
5. Fall back to defaults if invalid

### Programs and Profiles

**Program:** Sequence of jar/profile pairs (e.g., "Full Clean")
**Profile:** Behavior in a single jar (RPM, time, direction, temperature)

```toml
[program.full_clean]
label = "Full Clean"
steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
    { jar = "dry",    profile = "dry" },
]

[profile.clean]
rpm = 120
time_s = 180
direction = "alternate"
iterations = 3
```

---

## Memory Layout

```
┌──────────────────────┐ 0x10000000
│ Vector Table         │
├──────────────────────┤
│ .text (code)         │
├──────────────────────┤
│ .rodata (constants)  │
├──────────────────────┤
│ .data (initialized)  │
├──────────────────────┤
│ .bss (zero-init)     │
├──────────────────────┤
│ Heap (32KB)          │ ← TOML parsing
├──────────────────────┤
│ Stack                │
├──────────────────────┤ 0x100F0000
│ Config Flash (64KB)  │ ← TOML storage
└──────────────────────┘ 0x10100000
```

The 32KB heap is used for TOML parsing at boot. After config is loaded into static memory, heap usage is minimal.

---

## Testing Strategy

### Host Tests (cargo test)

- `isochron-core`: State machine, scheduler, motion math
- `isochron-protocol`: Frame encoding/decoding
- `isochron-drivers`: TMC2209 datagram generation/parsing

### Hardware Tests (probe-rs)

- Flash and run with RTT logging: `cargo run`
- Set log level in `.cargo/config.toml`: `DEFMT_LOG = "debug"`
- View output via probe-rs RTT

### Test Coverage

| Crate | Tests | Purpose |
|-------|-------|---------|
| isochron-core | 40 | State transitions, scheduler, motion |
| isochron-protocol | 24 | Frame parsing, message encoding |
| isochron-drivers | 7 | TMC2209 datagram, CRC |
