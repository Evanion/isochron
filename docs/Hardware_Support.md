# Hardware Support

This document lists supported hardware and compatibility information.

## Controller Boards

### Fully Supported

| Board | MCU | HAL | Status |
|-------|-----|-----|--------|
| BTT SKR Pico | RP2040 | isochron-hal-rp2040 | Supported |
| Raspberry Pi Pico | RP2040 | isochron-hal-rp2040 | Supported |

### Compatible (untested)

These boards use the RP2040 and should work with appropriate pin configuration.

**With external stepper drivers:**

| Board | Notes |
|-------|-------|
| Adafruit Feather RP2040 | Fewer GPIOs, needs external driver board |
| Adafruit ItsyBitsy RP2040 | Compact, limited pins |
| Pimoroni Tiny 2040 | Very compact |
| Seeed XIAO RP2040 | Minimal pin count |

These boards require external stepper driver modules (A4988, DRV8825, TMC2209) connected via step/dir/enable pins.

**Simple motor support**

For machines using basic DC or AC motors (no stepper drivers), these compact boards are ideal. See [Motor Types](#motor-types) below.

### Planned

| Board | MCU | HAL | Notes |
|-------|-----|-----|-------|
| FYSETC Spider | STM32F446 | isochron-hal-stm32f4 | Future |

### Not Supported

| Board | Reason |
|-------|--------|
| Arduino Uno/Nano | AVR architecture, no Embassy support |
| Pro Trinket | AVR architecture |
| ESP8266 | Limited resources, different ecosystem |

## Display Modules

### Fully Supported

| Display | MCU | Connection | Status |
|---------|-----|------------|--------|
| Direct I2C OLED | - | I2C to controller | Supported |

### Planned

| Display | MCU | Connection | Notes |
|---------|-----|------------|-------|
| V0 Mini OLED | STM32F042 | UART | Requires embassy migration |
| TFT35 SPI | - | SPI to controller | Direct drive |

## Pin Requirements

### Minimum Configuration

For a basic watch cleaner, you need:

| Function | Pins Required | Notes |
|----------|---------------|-------|
| Stepper motor | 3 (step, dir, enable) | For basket rotation |
| Heater | 1 (PWM capable) | For cleaning solution |
| Thermistor | 1 (ADC) | Temperature sensing |
| **Total** | **5 pins** | |

### Full Configuration

| Function | Pins Required | Notes |
|----------|---------------|-------|
| Stepper motor | 3 | step, dir, enable |
| Heater | 1-2 | PWM for control |
| Thermistors | 2-3 | ADC pins |
| Fans | 1-2 | PWM for speed control |
| Display UART | 2 | TX, RX |
| Status LED | 1 | Optional Neopixel |
| **Total** | **~12 pins** | |

## Board-Specific Notes

### BTT SKR Pico

The BTT SKR Pico is the reference board, designed for 3D printers but perfect for watch cleaners:

**Advantages:**
- Integrated TMC2209 stepper drivers
- Multiple thermistor inputs
- Heater outputs with MOSFETs
- UART header for display
- Well-documented pinout

**Pin Mapping:** See `configs/boards/btt-pico.toml`

### Raspberry Pi Pico

Standard development board, requires external components:

**Advantages:**
- Low cost
- Widely available
- Good documentation

**Requirements:**
- External stepper driver (A4988, TMC2209, etc.)
- MOSFET module for heater
- Voltage divider for thermistor

**Pin Mapping:** See `configs/boards/pico.toml`

### Custom Boards

Create your own configuration:

1. Copy `configs/boards/custom-rp2040.toml.example`
2. Rename to your board name
3. Update pin assignments
4. Create a profile or use directly

## Stepper Drivers

Isochron supports any stepper driver with step/direction interface:

| Driver | Voltage | Features |
|--------|---------|----------|
| A4988 | 8-35V | Basic, loud |
| DRV8825 | 8.2-45V | Higher current |
| TMC2208 | 4.75-36V | Silent, UART config |
| TMC2209 | 4.75-29V | Silent, stallguard |

The BTT SKR Pico has integrated TMC2209 drivers.

## Motor Types

Isochron supports three motor types to accommodate different build requirements and budgets.

### Stepper Motors

Stepper motors with step/direction interface provide:
- Precise position control
- Variable speed during cycles
- Smooth acceleration/deceleration
- StallGuard sensorless homing (with TMC drivers)

Use the `pico-stepper` or `btt-pico` profiles for stepper-based builds.

### DC Motors

DC motors with PWM speed control provide:
- Variable speed via duty cycle (0-100%)
- Soft start/stop ramping for smooth operation
- Lower cost than stepper systems
- Simple wiring with H-bridge drivers (L298N, TB6612)

| Feature | Details |
|---------|---------|
| Speed Control | PWM duty cycle 0-100% |
| Direction | Via H-bridge or dual MOSFET |
| Ramp Control | Configurable soft start/stop (ms) |
| Min Duty | Configurable minimum to overcome static friction |

Use the `pico-dc-motor` profile for DC motor builds.

### AC Motors

AC motors with relay control provide:
- Fixed-speed operation (no speed control)
- Simple wiring with SSR or mechanical relays
- Ideal for retrofitting existing ultrasonic cleaners
- Industrial/high-power applications

| Feature | Details |
|---------|---------|
| Speed Control | None (fixed speed) |
| Direction | Optional via reversing relay |
| Safety | Configurable switch delay to protect relays |
| Interlock | Prevents multiple motors running simultaneously |

Use the `pico-ac-motor` profile for AC motor builds.

### Configuration Examples

**DC Motor** (`configs/machines/examples/dc-motor-basic.toml`):

```toml
[machine]
motor_type = "dc"

[dc_motor.basket]
pwm_pin = "gpio11"
dir_pin = "gpio10"
enable_pin = "!gpio12"
driver_type = "h_bridge"
pwm_frequency = 25000
min_duty = 20
soft_start_ms = 500
soft_stop_ms = 300
```

**AC Motor** (`configs/machines/examples/ac-motor-basic.toml`):

```toml
[machine]
motor_type = "ac"

[ac_motor.basket]
enable_pin = "gpio11"
direction_pin = "gpio10"  # Optional
relay_type = "ssr"
active_high = true

[ac_safety]
min_switch_delay_ms = 100
interlock_enabled = true
```

## Temperature Sensors

### Thermistors

Standard 100K NTC thermistors work out of the box:

- EPCOS 100K (common in 3D printers)
- Generic 100K NTC
- Semitec 104GT-2

Configure in your machine config with appropriate beta value.

### Thermocouples

Not currently supported. Could be added with MAX31855/MAX6675 driver.

## Power Requirements

| Component | Typical Current | Notes |
|-----------|-----------------|-------|
| RP2040 board | 50-100mA | 3.3V logic |
| Stepper (per) | 0.5-2A | Depends on driver setting |
| Heater | 2-5A | 12V or 24V heating element |
| Fan (per) | 100-200mA | 12V fans |

**Recommended power supply:** 12V or 24V, 5A minimum

## Wiring Guidelines

1. **Keep signal wires away from power wires** to reduce noise
2. **Use shielded cables** for thermistor connections
3. **Add flyback diodes** if using relay modules
4. **Use appropriate wire gauge** for heater current (18-20 AWG for 5A)
5. **Ensure good ground connections** - use star grounding if possible
