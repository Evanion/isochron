# Configuration Reference

This document describes the available configuration sections and parameters for the Isochron firmware. Configuration follows a Klipper-inspired TOML format.

## Overview

Isochron uses a hierarchical configuration with these main sections:

- **Hardware sections**: `[stepper]`, `[tmc2209]`, `[heater]`, `[display]`
- **Machine sections**: `[jar]`, `[profile]`, `[program]`
- **UI sections**: `[ui]`

Parameters shown with `#` prefix are optional with default values. Parameters without `#` are required.

---

## Stepper Configuration

### [stepper name]

Defines a stepper motor. The `name` identifies this stepper (e.g., `basket`, `z`, `x`).

```toml
[stepper basket]
step_pin = "gpio11"
#   The GPIO pin for step pulses. This parameter must be provided.

dir_pin = "gpio10"
#   The GPIO pin for direction control. This parameter must be provided.

enable_pin = "!gpio12"
#   The GPIO pin for motor enable. Prefix with ! for active-low (inverted).
#   This parameter must be provided.

#endstop_pin = "^gpio4"
#   The GPIO pin for the endstop switch. Prefix with ^ for pull-up.
#   Only required for position-controlled steppers (z, x).
#   Can also use "tmc2209_name:virtual_endstop" for sensorless homing.

#full_steps_per_rotation = 200
#   The number of full steps for one motor rotation. Use 200 for 1.8°
#   motors or 400 for 0.9° motors. The default is 200.

#microsteps = 16
#   The microstep setting for the stepper driver. Common values are
#   8, 16, 32, or 64. The default is 16.

#rotation_distance = 360
#   The distance traveled (in mm or degrees) per full motor rotation.
#   For the basket motor, this is typically 360 (degrees).
#   For linear axes, this is the leadscrew pitch (mm).
#   The default is 360.

#gear_ratio = "3:1"
#   The gear ratio between motor and output. Format is "driven:driving".
#   For example, "3:1" means motor turns 3 times for 1 output rotation.
#   The default is "1:1" (direct drive).
```

#### Pin Syntax

Pins can be specified with modifiers:

| Syntax | Meaning |
|--------|---------|
| `gpio11` | Normal GPIO 11 |
| `!gpio12` | Inverted (active-low) GPIO 12 |
| `^gpio4` | GPIO 4 with internal pull-up |
| `^!gpio4` | Inverted with pull-up |

#### Stepper Names

Reserved stepper names with special behavior:

| Name | Purpose | Required |
|------|---------|----------|
| `basket` | Basket rotation motor | Yes |
| `z` | Basket vertical movement (lift) | Optional |
| `x` | Basket horizontal positioning (jar selection) | Optional |
| `lid` | Drying chamber lid | Optional |

**Note:** A machine with both `z` and `x` steppers is considered "automated" - the firmware controls basket movement between jars.

---

## TMC2209 Configuration

### [tmc2209 name]

Configures a TMC2209 stepper driver. The `name` must match a `[stepper name]`.

```toml
[tmc2209 basket]
#   Configure TMC2209 for the "basket" stepper.

uart_tx_pin = "gpio8"
#   UART transmit pin. This parameter must be provided.

uart_rx_pin = "gpio9"
#   UART receive pin. This parameter must be provided.

#uart_address = 0
#   UART address for multi-driver bus (0-3). Each driver on the same
#   UART bus must have a unique address. The default is 0.

#run_current = 0.8
#   Motor RMS current (in Amps) during movement. Higher values provide
#   more torque but generate more heat. The default is 0.8.

#hold_current = 0.4
#   Motor RMS current (in Amps) when stationary. Can be lower than
#   run_current to reduce heat. The default is half of run_current.

#stealthchop = true
#   Enable StealthChop mode for quiet operation. Disable for higher
#   speeds or when StallGuard is needed. The default is true.

#stallguard_threshold = 80
#   StallGuard sensitivity (0-255). Higher values = more sensitive.
#   Used for stall detection and sensorless homing. The default is 80.

#diag_pin = "gpio17"
#   DIAG pin for StallGuard output. Optional - only needed if using
#   stall detection or sensorless homing.
```

#### Multi-Driver UART Bus

Multiple TMC2209 drivers can share a single UART bus using different addresses:

```toml
[tmc2209 basket]
uart_tx_pin = "gpio8"
uart_rx_pin = "gpio9"
uart_address = 0

[tmc2209 z]
uart_tx_pin = "gpio8"
uart_rx_pin = "gpio9"
uart_address = 1

[tmc2209 x]
uart_tx_pin = "gpio8"
uart_rx_pin = "gpio9"
uart_address = 2
```

---

## Heater Configuration

### [heater name]

Defines a heater with temperature control.

```toml
[heater dryer]
#   Configure heater named "dryer".

heater_pin = "gpio23"
#   The GPIO pin controlling the heater (via SSR or MOSFET).
#   This parameter must be provided.

sensor_pin = "gpio27"
#   The ADC pin connected to the temperature sensor.
#   This parameter must be provided.

#sensor_type = "ntc100k"
#   The type of temperature sensor. Options:
#   - "ntc100k": NTC 100K thermistor (most common)
#   - "ntc10k": NTC 10K thermistor
#   - "pt100": PT100 RTD (future support)
#   The default is "ntc100k".

#control = "bang_bang"
#   The control algorithm. Options:
#   - "bang_bang": Simple on/off with hysteresis
#   - "pid": PID control with time-proportioning output
#   The default is "bang_bang".

#max_temp = 55
#   Maximum allowed temperature in °C. The heater will shut off and
#   trigger a fault if this temperature is exceeded. The default is 55.

#hysteresis = 2
#   Temperature hysteresis in °C for bang-bang control. Heater turns
#   off at target, turns on at (target - hysteresis). The default is 2.
#   Only used when control = "bang_bang".

#pid_kp = 1.5
#   PID proportional gain. Controls response to current error.
#   Higher values = faster response but may overshoot.
#   Only used when control = "pid". Can be set manually or via autotune.

#pid_ki = 0.1
#   PID integral gain. Eliminates steady-state error over time.
#   Higher values = faster error correction but may cause oscillation.
#   Only used when control = "pid". Can be set manually or via autotune.

#pid_kd = 0.5
#   PID derivative gain. Dampens oscillation and improves stability.
#   Higher values = more damping but may slow response.
#   Only used when control = "pid". Can be set manually or via autotune.
```

#### PID Control

When `control = "pid"` is set, the heater uses a PID controller with time-proportioning output. Since the heater is on/off (not PWM capable), the controller modulates the duty cycle over a 10-second period.

**PID Coefficient Priority:**
1. Values in TOML config (highest priority - manual override)
2. Values saved in flash from autotune
3. Default values (zeros - effectively disables PID output)

**Coefficient Format:**
PID coefficients can be specified as:
- Float values: `pid_kp = 1.5`
- Scaled integers (×100): `pid_kp = 150` (equivalent to 1.50)

#### Autotune

The firmware includes an autotune feature that automatically determines optimal PID coefficients using the Åström-Hägglund relay method with Ziegler-Nichols tuning rules.

**Autotune Process:**
1. Navigate to "Autotune Heater" in the menu
2. Confirm the target temperature (default: 45°C)
3. The heater oscillates around the setpoint while collecting data
4. After 12+ oscillation peaks, coefficients are calculated
5. Results can be saved to flash for persistent storage

**Autotune Duration:** Typically 5-15 minutes depending on heater/thermal mass.

**Autotune Abort Conditions:**
- Temperature exceeds `max_temp` (safety cutoff)
- Timeout after 20 minutes
- Sensor fault detected
- User cancellation (long-press encoder)

#### Heater Safety

The firmware monitors heaters for safety:

- **Over-temperature**: Triggers fault if temp exceeds `max_temp`
- **Sensor fault**: Triggers fault if sensor reads open/short circuit
- **State enforcement**: Heater only operates in `Running` or `Autotuning` states
- **Thermal fuse**: Hardware backup recommended (see Machine Design guide)

---

## DC Motor Configuration

### [dc_motor name]

Defines a DC motor with PWM speed control. Use this instead of `[stepper]` for simple DC motors.

```toml
[dc_motor basket]
#   Configure DC motor named "basket".

pwm_pin = "gpio11"
#   The GPIO pin for PWM output. This parameter must be provided.

#dir_pin = "gpio10"
#   The GPIO pin for direction control. Optional - omit for
#   unidirectional motors.

#enable_pin = "!gpio12"
#   The GPIO pin for motor enable. Prefix with ! for active-low.
#   Optional - omit if motor runs whenever PWM is applied.

#pwm_frequency = 25000
#   PWM frequency in Hz. Higher frequencies are quieter but may
#   reduce efficiency. The default is 25000 (25kHz).

#min_duty = 20
#   Minimum duty cycle percentage (0-100). Motor won't turn below
#   this threshold. The default is 20.

#soft_start_ms = 500
#   Ramp-up time in milliseconds from 0 to target speed.
#   Reduces mechanical stress. The default is 500.

#soft_stop_ms = 300
#   Ramp-down time in milliseconds from current speed to 0.
#   The default is 300.
```

#### DC Motor Speed Control

For DC motors, the `rpm` value in profiles is interpreted as a speed percentage (0-100), not actual RPM:

```toml
[profile clean]
rpm = 75  # 75% duty cycle for DC motors
```

---

## AC Motor Configuration

### [ac_motor name]

Defines an AC motor with relay control. Use this for AC induction motors that run at fixed speed.

```toml
[ac_motor basket]
#   Configure AC motor named "basket".

relay_pin = "gpio12"
#   The GPIO pin controlling the motor relay/contactor.
#   This parameter must be provided.

#dir_pin = "gpio10"
#   The GPIO pin for direction control (reversing contactor).
#   Optional - omit for unidirectional motors.

#active_high = true
#   Set to false if the relay is active-low. The default is true.

#relay_type = "mechanical"
#   Type of relay. Options:
#   - "mechanical": Standard relay (needs switch delay)
#   - "ssr": Solid-state relay (faster switching)
#   The default is "mechanical".

#min_switch_delay_ms = 100
#   Minimum time between relay state changes (milliseconds).
#   Prevents rapid switching that can damage contactors.
#   The default is 100.
```

#### AC Motor Speed

AC motors run at fixed speed determined by line frequency and motor poles. The `rpm` value in profiles is interpreted as on (> 0) or off (0):

```toml
[profile clean]
rpm = 1  # Any non-zero value turns AC motor on
```

---

## Motor Type Selection

Set the motor type at the top of your configuration:

```toml
motor_type = "stepper"  # Options: "stepper", "dc", "ac"
```

| Type | Driver Section | Speed Control |
|------|----------------|---------------|
| `stepper` | `[stepper]` + `[tmc2209]` | Precise RPM via microstepping |
| `dc` | `[dc_motor]` | PWM duty cycle (0-100%) |
| `ac` | `[ac_motor]` | On/off only |

---

## Display Configuration

### [display]

Configures the V0 Display communication.

```toml
[display]
#type = "v0_display"
#   Display type. Currently only "v0_display" is supported.
#   The default is "v0_display".

uart_tx_pin = "gpio0"
#   UART transmit pin to display. This parameter must be provided.

uart_rx_pin = "gpio1"
#   UART receive pin from display. This parameter must be provided.

#baud = 115200
#   UART baud rate. Must match display firmware settings.
#   The default is 115200.
```

---

## Jar Configuration

### [jar name]

Defines a jar (cleaning station) in the machine.

```toml
[jar clean]
#   Define jar named "clean".

#x_pos = 0
#   Position in degrees from home for the x motor (jar selection).
#   Only used on automated machines with x stepper.
#   The default is 0.

#z_pos = 120
#   Position in mm from top for the z motor (vertical lift).
#   Only used on automated machines with z stepper.
#   The default is 0.

#heater = "dryer"
#   Name of the heater associated with this jar.
#   When a profile runs in this jar with a temperature target,
#   this heater will be activated. Optional.

#ultrasonic = "us_clean"
#   Name of the ultrasonic module for this jar. Optional.
#   (Future feature)
```

#### Standard Jar Names

Typical configurations use these jar names:

| Name | Purpose |
|------|---------|
| `clean` | Cleaning solution (first step) |
| `rinse` or `rinse1` | First rinse |
| `rinse2` | Second rinse (optional) |
| `dry` | Drying chamber (usually with heater) |

---

## Profile Configuration

### [profile name]

Defines a cleaning profile (behavior in a single jar).

```toml
[profile clean]
#   Define profile named "clean".

label = "Clean"
#   Display label shown in the UI. This parameter must be provided.

#type = "clean"
#   Profile type for categorization. Options: "clean", "rinse", "dry".
#   The default is "clean".

#rpm = 120
#   Target rotation speed in RPM. The default is 120.

#time_s = 180
#   Total duration in seconds. The default is 180 (3 minutes).

#direction = "alternate"
#   Rotation direction mode. Options:
#   - "cw" or "clockwise": Continuous clockwise
#   - "ccw" or "counterclockwise": Continuous counter-clockwise
#   - "alternate": Alternates between CW and CCW
#   The default is "alternate".

#iterations = 3
#   Number of direction changes for "alternate" mode. Each iteration
#   is one CW + one CCW cycle. Only used with "alternate" direction.
#   The default is 3.

#temperature_c = 45
#   Target temperature in °C. If specified, the jar's heater will be
#   activated to maintain this temperature. Optional - omit for no heating.
```

### [profile.name.spinoff]

Optional spin-off phase after the main profile completes. Spin-off removes excess solution from the basket before moving to the next jar.

```toml
[profile.clean.spinoff]
#   Spin-off configuration for the "clean" profile.

#lift_mm = 20
#   Height in mm to lift the basket above the jar before spinning.
#   Only used on automated machines. The default is 20.

#rpm = 150
#   Spin speed during spin-off. Higher than normal for better
#   solution removal. The default is 150.

#time_s = 10
#   Duration of spin-off in seconds. The default is 10.
```

**Note:** On manual machines, the user is prompted to lift the basket before spin-off begins.

---

## Program Configuration

### [program name]

Defines a cleaning program (sequence of jar/profile steps).

```toml
[program full_clean]
#   Define program named "full_clean".

label = "Full Clean"
#   Display label shown in the UI. This parameter must be provided.

steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
    { jar = "rinse2", profile = "rinse" },
    { jar = "dry",    profile = "dry" },
]
#   Ordered list of steps. Each step specifies a jar and profile.
#   The program executes steps in order. This parameter must be provided.
```

#### Step Execution

For each step:

1. **Automated machines**: Firmware moves basket to jar position
2. **Manual machines**: Display prompts user to move basket
3. Profile executes (spin at specified RPM, optional heating)
4. Spin-off phase runs (if configured in profile)
5. Proceed to next step

---

## UI Configuration

### [ui]

Configures user interface behavior.

```toml
[ui]
#rpm_step = 10
#   Increment/decrement step when adjusting RPM. The default is 10.

#time_step_s = 30
#   Increment/decrement step when adjusting time (seconds).
#   The default is 30.

#temp_step_c = 5
#   Increment/decrement step when adjusting temperature (°C).
#   The default is 5.
```

---

## Complete Example Configuration

Here's a complete configuration for a manual 4-jar watch cleaning machine:

```toml
# machine.toml - Isochron Watch Cleaner Configuration

# === STEPPERS ===

[stepper basket]
step_pin = "gpio11"
dir_pin = "gpio10"
enable_pin = "!gpio12"
full_steps_per_rotation = 200
microsteps = 16
rotation_distance = 360
gear_ratio = "3:1"

[tmc2209 basket]
uart_tx_pin = "gpio8"
uart_rx_pin = "gpio9"
uart_address = 0
run_current = 0.8
hold_current = 0.4
stealthchop = true
diag_pin = "gpio17"

# === HEATERS ===

[heater dryer]
heater_pin = "gpio23"
sensor_pin = "gpio27"
sensor_type = "ntc100k"
control = "pid"           # Use PID for better temperature stability
max_temp = 55
# PID coefficients - run autotune to determine optimal values
# pid_kp = 1.5
# pid_ki = 0.1
# pid_kd = 0.5

# === DISPLAY ===

[display]
uart_tx_pin = "gpio0"
uart_rx_pin = "gpio1"
baud = 115200

# === JARS ===

[jar clean]
x_pos = 0
z_pos = 0

[jar rinse1]
x_pos = 0
z_pos = 0

[jar rinse2]
x_pos = 0
z_pos = 0

[jar dry]
x_pos = 0
z_pos = 0
heater = "dryer"

# === PROFILES ===

[profile clean]
label = "Clean"
type = "clean"
rpm = 120
time_s = 180
direction = "alternate"
iterations = 3

[profile.clean.spinoff]
lift_mm = 20
rpm = 150
time_s = 10

[profile rinse]
label = "Rinse"
type = "rinse"
rpm = 100
time_s = 120
direction = "cw"

[profile.rinse.spinoff]
lift_mm = 20
rpm = 150
time_s = 8

[profile dry]
label = "Dry"
type = "dry"
rpm = 60
time_s = 600
direction = "alternate"
iterations = 6
temperature_c = 45

# === PROGRAMS ===

[program full_clean]
label = "Full Clean"
steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
    { jar = "rinse2", profile = "rinse" },
    { jar = "dry",    profile = "dry" },
]

[program quick_clean]
label = "Quick Clean"
steps = [
    { jar = "clean",  profile = "clean" },
    { jar = "rinse1", profile = "rinse" },
]

[program dry_only]
label = "Dry Only"
steps = [
    { jar = "dry", profile = "dry" },
]

# === UI ===

[ui]
rpm_step = 10
time_step_s = 30
temp_step_c = 5
```

---

## Configuration Storage

Configuration is stored in flash memory:

- **Location**: Last 64KB of flash (config partition)
- **Format**: TOML text (parsed at boot)
- **Fallback**: Built-in defaults if config is invalid

### Updating Configuration

Configuration can be updated via:

1. **USB Mass Storage** (future): Edit config file directly
2. **Serial Console** (future): Send new config via UART
3. **Reflash**: Rebuild firmware with new embedded defaults

---

## Validation

The firmware validates configuration at boot:

| Check | Action on Failure |
|-------|-------------------|
| Required stepper "basket" missing | Use defaults |
| Profile references unknown jar | Skip profile |
| Program references unknown profile | Skip step |
| Invalid pin number | Use defaults |
| Temperature > max_temp | Clamp to max |

Invalid configurations log warnings but don't prevent boot.
