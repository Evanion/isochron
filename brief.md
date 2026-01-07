Isochron – Watch Cleaning Machine Controller

Project Brief – Stepper‑Driven Watch Cleaning Machine

1. Purpose

Design and build a precision, low‑RPM, programmable drive system for a jar‑based mechanical watch cleaning setup. The goal is to achieve repeatable, gentle, and solvent‑safe agitation comparable to professional watch cleaning machines, using readily available components and custom firmware.

⸻

2. Design Goals

Primary goals
	•	Stable rotation at 60–150 RPM
	•	Very smooth motion (no cogging, no surging)
	•	Programmable cleaning cycles (clean / rinse / final rinse)
	•	Optional alternating direction
	•	Safe operation around volatile solvents

Secondary goals
	•	Simple, robust hardware stack
	•	Minimal unnecessary complexity
	•	Expandable (heater, lid interlock, temperature sensing)
	•	Appliance‑like usability with rotary encoder + screen

⸻

3. Mechanical Architecture (Summary)
	•	Drive motor: NEMA 14 bipolar stepper
	•	Transmission: GT2 belt reduction (6 mm)
	•	Target ratio: 3:1 – 4:1
	•	Output: vertical shaft with thrust bearing
	•	Load: lightweight watch basket in 1.2 L glass jars

Key mechanical principles:
	•	Motor must never carry axial load
	•	Belt used for vibration isolation and smoothness
	•	All moving parts above solvent level

⸻

4. Electronics Architecture

Core control
	•	SKR Pico (RP2040)
	•	Stepper motor control (TMC drivers)
	•	Real‑time motion generation
	•	Safety inputs (lid switch / emergency stop)

User interface
	•	Voron V0 screen
	•	Rotary encoder (primary input)
	•	Click action (confirm / start)
	•	Reset / kill button
	•	Small I²C display

Power
	•	24 V DC power supply
	•	Fused input
	•	Common ground reference

⸻

5. Software Architecture – Key Question

Decision to make

Should the system use a Raspberry Pi Zero, or run standalone directly on the SKR Pico?

⸻

6. Software Option A – Standalone SKR Pico (Recommended)

Description

Custom firmware runs entirely on the RP2040. The SKR Pico directly controls:
	•	Stepper motor
	•	Timing and motion profiles
	•	Encoder input
	•	Screen output
	•	Safety logic

Advantages
	•	Deterministic real‑time behavior
	•	No OS, no boot delays
	•	Much lower complexity
	•	Higher reliability near solvents
	•	Appliance‑like feel

Responsibilities of firmware
	•	State machine for cleaning cycles
	•	RPM → step rate mapping
	•	Acceleration / deceleration ramps
	•	Direction control
	•	UI menu system
	•	Profile storage (flash)

When this is ideal
	•	Fixed‑function machine
	•	Physical controls only
	•	No network requirements

⸻

7. Software Option B – Raspberry Pi Zero + SKR Pico

Description

Raspberry Pi Zero acts as a host controller. SKR Pico becomes a motion coprocessor.

Typical split
	•	Pi Zero:
	•	UI logic
	•	Profile editing
	•	File storage
	•	Networking / web UI (optional)
	•	SKR Pico:
	•	Step generation
	•	Safety‑critical motion

Advantages
	•	Rich UI possible
	•	Remote control / monitoring
	•	Easier future feature growth

Disadvantages
	•	Higher complexity
	•	Boot time and OS maintenance
	•	More failure modes
	•	Overkill for current requirements

⸻

8. Recommendation

Use the SKR Pico standalone. Do NOT add a Raspberry Pi Zero initially.

Rationale:
	•	All real‑time requirements are easily met by RP2040
	•	UI requirements are modest
	•	Safety and reliability are improved
	•	System remains simple and robust

Design the firmware so that:
	•	A host connection could be added later
	•	Motion control remains self‑contained

⸻

9. Motion Profile Format (Critical)

Purpose

Define a strict, data-driven profile format used to describe cleaning, rinsing, and drying behavior. Profiles are declarative and safe; they express intent, while firmware enforces limits and sequencing.

Profiles are loaded from profiles.toml and validated at boot.

⸻

Profile structure

Each profile consists of one or more ordered steps executed sequentially.

Conceptual model:

Profile
 ├── label (string)
 ├── type (clean | rinse | dry)
 └── steps[]
       ├── motion parameters
       ├── timing parameters
       └── optional heater parameters


⸻

Step data structure

Each step has the following fields:

Field	Type	Description	Constraints
rpm	i16	Target basket speed	0–250 RPM (hard limit)
duration_s	u16	Step duration in seconds	1–3600
direction	enum	cw, ccw, or alternate	default: cw
pause_ms	u16	Optional pause before direction change	0–5000 ms
heater	optional	Heater config (drying only)	see below


⸻

Heater sub-structure (optional)

Only valid for profiles of type dry.

Field	Type	Description	Constraints
target_c	i16	Desired temperature	30–50 °C
max_c	i16	Absolute cutoff temperature	≤ 55 °C

If heater parameters are present in a non-drying profile, the profile is rejected.

⸻

Direction behavior
	•	cw / ccw: continuous rotation in one direction
	•	alternate: direction toggles automatically
	•	Toggle interval = step duration / number of alternations
	•	Firmware enforces minimum segment length (≥10 s)

On every direction change:
	•	Motor decelerates to 0 RPM
	•	Optional pause_ms dwell
	•	Direction reverses
	•	Motor accelerates smoothly to target RPM

⸻

Acceleration & deceleration

Acceleration is not profile-defined to prevent unsafe configurations.

Firmware applies global motion constraints:
	•	Linear ramp
	•	Default acceleration: 50–100 RPM/s
	•	Symmetric accel/decel
	•	No instantaneous direction reversal

This guarantees:
	•	Minimal fluid shock
	•	Reduced vortex formation
	•	Mechanical longevity

⸻

Profile chaining

Profiles are not self-chaining.

Instead, the UI presents logical workflow groups:

Example:
	•	“Full Clean” → internally runs:
	1.	Clean profile
	2.	Rinse 1 profile
	3.	Rinse 2 profile

Chaining rules:
	•	Defined in firmware, not user config
	•	Prevents unsafe or illogical sequences
	•	Allows firmware to insert mandatory stops or user prompts

⸻

Validation rules (boot-time)
	•	Maximum steps per profile: 16
	•	Total profile runtime ≤ 90 minutes
	•	RPM, duration, temperature clamped to safe ranges
	•	Invalid profiles ignored with error shown on screen
	•	Built-in default profiles always available

⸻

10. UI Menu Structure (Critical)

Design goals
	•	Extremely fast start for common workflows
	•	No mandatory parameter editing
	•	All parameters visible before start
	•	Rotary encoder only (scroll + click)
	•	No nested complexity beyond one level

The UI is profile-centric, not parameter-centric.

⸻

Top-level screen

On boot / idle, the screen shows a simple list:

> Clean
  Rinse
  Dry

	•	Encoder scrolls profiles
	•	Click selects profile

⸻

Profile detail screen

After selecting a profile, the user is shown the profile summary, with START preselected:

Clean
RPM:        120
Time:       3:00
Direction:  Alternate
Iterations: 3

> START

	•	Encoder scrolls fields
	•	Click enters edit mode for a field
	•	START is always the last item and preselected

A second click on START begins execution immediately.

⸻

Editing behavior

RPM
	•	Editable per profile
	•	Increment step: 5 or 10 RPM (global setting)
	•	Default: 10 RPM
	•	Displayed as integer RPM
	•	Firmware clamps to absolute safe range (e.g. 0–250 RPM)

Time
	•	Editable in fixed steps (global setting):
	•	15 s or
	•	30 s or
	•	60 s
	•	Step size chosen at compile-time or in global config
	•	Displayed as mm:ss

Temperature (drying profiles only)
	•	Editable in 5 °C increments
	•	Clamped to firmware-safe range

⸻

Direction model (UI simplification)

Direction behavior is simplified intentionally to reduce user error and macro complexity.

Instead of exposing raw direction steps, the UI presents:
	•	Direction mode:
	•	CW
	•	CCW
	•	Alternate
	•	Iterations (only visible if Alternate selected)
	•	Number of back-and-forth cycles
	•	Each iteration consists of:
	•	Forward segment
	•	Reverse segment

Segment duration is computed automatically:

segment_time = total_time / (iterations * 2)

Firmware enforces:
	•	Minimum segment duration (e.g. ≥10 s)
	•	Automatic decel → pause → reverse → accel

⸻

Runtime screen

While a profile is running, the UI switches to a dedicated status screen.

Running: Clean
RPM: 120
Direction: Alternate
Time left: 1:42

> PAUSE / STOP

	•	The currently running profile label is always shown
	•	Remaining time is shown as mm:ss
	•	Direction mode is informational only (not editable)

PAUSE / STOP is always preselected.

⸻

Runtime input behavior
	•	Encoder click:
	•	Immediately stops motion and heater output
	•	Transitions system to PAUSED state
	•	Encoder long-press:
	•	Immediate abort
	•	Motor and heater disabled
	•	Scheduler reset
	•	Transition to IDLE

This ensures that a single, fast user action always stops the machine.

⸻

Error presentation
	•	Errors are modal and block operation
	•	Screen shows:
	•	Short error description
	•	Required user action (e.g. “Press to acknowledge”)
	•	After acknowledgement, system returns to IDLE

⸻

11. SKR Pico Pin Mapping (Critical)

Scope

Pin mapping is board- and wiring-specific and is therefore defined in config.toml.
A default boilerplate configuration is shipped with the firmware.

Motor control pins and TMC driver wiring are fixed by the SKR Pico hardware and are not user-configurable.

⸻

Configurable pins (via config.toml)
	•	Display UART TX / RX
	•	Heater output GPIO
	•	Thermistor ADC input

Note: Encoder is handled by the V0 Display (STM32F042), not the SKR Pico.

⸻

Non-configurable pins (firmware-owned)
	•	Stepper STEP / DIR / ENABLE
	•	TMC UART pins
	•	Motor driver power and reference voltages

⸻

12. TMC Driver Configuration (Fixed)

Driver variant
	•	TMC2209 (as populated on SKR Pico)
	•	UART-controlled

Fixed firmware settings
	•	Microstepping: 16× or 32× (selected at compile-time)
	•	RMS motor current: ~0.8–1.0 A
	•	Mode: StealthChop enabled
	•	Interpolation enabled

These parameters are not exposed to user configuration.

⸻

13. State Machine Details (Critical)

This section defines the authoritative runtime behavior of the machine. The state machine is explicit, finite, and deterministic. All motor, heater, and UI behavior must be explainable as a function of the current state and an event.

The state machine is designed to be:
	•	Directly translatable to a Rust enum
	•	Safe by construction
	•	Resistant to partial failures (bad config, sensor faults)

⸻

State definitions

State	Description
BOOT	Power-on initialization, hardware checks, config loading
IDLE	Ready state, profile list visible
PROFILE_SELECTED	Profile chosen, summary displayed
EDIT_PROFILE	User editing profile parameters
RUNNING	Profile executing (motor + optional heater active)
PAUSED	Execution paused by user
COMPLETE	Profile finished successfully
ERROR	Fault detected; outputs disabled


⸻

Events

Event	Source
BootComplete	Firmware init
SelectProfile	Encoder click
EditParameter	Encoder click
ConfirmEdit	Encoder click
Start	Encoder click on START
Pause	Encoder click during RUNNING
Resume	Encoder click during PAUSED
Abort	Encoder long-press
StepFinished	Scheduler
ProfileFinished	Scheduler
ErrorDetected	Safety subsystem
AcknowledgeError	Encoder click


⸻

Transition table (core)

Current State	Event	Action	Next State
BOOT	BootComplete	Show profile list	IDLE
IDLE	SelectProfile	Load profile summary	PROFILE_SELECTED
PROFILE_SELECTED	EditParameter	Enter edit mode	EDIT_PROFILE
EDIT_PROFILE	ConfirmEdit	Apply + validate	PROFILE_SELECTED
PROFILE_SELECTED	Start	Initialize controllers	RUNNING
RUNNING	Pause	Decel motor, heater OFF	PAUSED
PAUSED	Resume	Resume controllers	RUNNING
RUNNING	StepFinished	Advance scheduler	RUNNING
RUNNING	ProfileFinished	Stop outputs	COMPLETE
COMPLETE	SelectProfile	Reset context	PROFILE_SELECTED
ANY	Abort	Stop outputs immediately	IDLE
ANY	ErrorDetected	Stop outputs immediately	ERROR
ERROR	AcknowledgeError	Clear fault	IDLE


⸻

RUNNING state internal behavior

While in RUNNING, the following subsystems operate concurrently:
	•	Motor controller
	•	Profile scheduler
	•	Heater controller (optional)
	•	Safety monitor

⸻

PAUSED state behavior
	•	Motor speed = 0 (controlled deceleration)
	•	Heater = OFF
	•	Timers frozen

⸻

ERROR state behavior
	•	Motor disabled immediately
	•	Heater disabled immediately
	•	Error message displayed

⸻

Design invariants
	•	Heater can only be ON in RUNNING
	•	Heater is never ON if motor speed = 0
	•	Motor direction never reverses without decel → pause → accel
	•	Invalid profiles can never reach RUNNING
	•	Any error forces transition to ERROR

⸻

Safety conditions triggering ERROR state

Condition               Detection method
────────────────────────────────────────────────────────────────
Thermistor fault        ADC reading out of valid range (open/short)
Over-temperature        Temperature > 55 °C
Motor stall             TMC2209 stallGuard flag (if enabled)
Display link lost       3 missed heartbeats + 3 failed retries

⸻

14. Profile Scheduler (Critical)

Purpose

The profile scheduler is responsible for converting a user-visible profile (RPM, total time, direction mode, iterations, heater settings) into a deterministic sequence of timed execution segments.

The scheduler:
	•	Owns all timekeeping for a profile
	•	Emits StepFinished and ProfileFinished events
	•	Is independent of UI and motor implementation details

⸻

Inputs (from profile)

Field	Description
rpm	Target rotation speed
total_time_s	Total runtime of profile
direction_mode	CW / CCW / Alternate
iterations	Number of alternations (Alternate only)
heater_config	Optional drying parameters


⸻

Derived execution model

The scheduler expands a profile into segments.

A segment is defined as:

Segment {
  direction
  duration_s
}


⸻

Segment generation rules

Single-direction profiles (CW / CCW)
	•	Exactly one segment is generated:

direction = CW or CCW
duration = total_time_s


⸻

Alternate-direction profiles
	•	Total number of segments = iterations × 2
	•	Segment duration is computed as:

segment_duration = total_time_s / (iterations × 2)

	•	Direction alternates every segment

Example (iterations = 3):

CW → CCW → CW → CCW → CW → CCW


⸻

Validation rules

Before execution, the scheduler validates:
	•	iterations > 0 if direction = Alternate
	•	segment_duration ≥ MIN_SEGMENT_TIME (e.g. 10 s)
	•	total_time_s divisible enough to maintain precision

If validation fails, the profile is rejected and cannot enter RUNNING.

⸻

Runtime behavior
	•	Scheduler maintains:
	•	Current segment index
	•	Elapsed time in segment
	•	Elapsed time in profile
	•	Time is tracked using a monotonic timer
	•	On segment completion:
	•	Emit StepFinished
	•	Advance to next segment
	•	On final segment completion:
	•	Emit ProfileFinished

⸻

Interaction with motor controller

For each segment:
	1.	Command motor direction
	2.	Accelerate motor to target RPM
	3.	Hold RPM for segment_duration
	4.	Decelerate motor to 0
	5.	Optional firmware-defined pause

Direction reversal never happens at speed.

⸻

Pause / resume semantics
	•	On Pause:
	•	Scheduler freezes time counters
	•	Current segment index preserved
	•	On Resume:
	•	Motor accelerates back to RPM
	•	Segment resumes from remaining time

⸻

Abort semantics
	•	Abort immediately stops scheduler
	•	All counters reset
	•	No ProfileFinished event emitted

⸻

Design invariants
	•	Scheduler is the single source of truth for timing
	•	UI never manipulates segment timing directly
	•	Motor controller never tracks elapsed time
	•	Heater controller reacts only to RUNNING state

⸻

15. Spin-Off Phase (Critical)

Purpose

After completing a cleaning or rinsing phase in a jar, excess solution remains on the basket and parts. If the basket is moved directly to the next jar, this solution will:
- Contaminate the fresh solution in the next jar
- Drip during transfer, causing mess

The spin-off phase addresses this by lifting the basket slightly above the jar and spinning at higher RPM to shed excess solution before transitioning to the next jar.

⸻

Spin-off behavior

The spin-off phase is an optional step that occurs after each profile completes (except drying, which is typically the final step).

Automated machines (with lift motor)
1. After profile completes, motor stops
2. Lift motor raises basket by configured `spinoff.lift_mm` (e.g., 20 mm)
3. Spin motor runs at `spinoff.rpm` for `spinoff.time_s`
4. Spin motor stops
5. Firmware proceeds to next step (either position move or AwaitingJar prompt)

Manual machines (no lift motor)
1. After profile completes, motor stops
2. Display shows: "Lift basket, press to continue"
3. User physically lifts basket above jar
4. User presses encoder to confirm
5. Spin motor runs at `spinoff.rpm` for `spinoff.time_s`
6. Spin motor stops
7. Display shows: "Move to next jar, press to continue"

⸻

Spin-off configuration (in profiles.toml)

```toml
[profile.clean]
label = "Clean"
rpm = 120
time_s = 180
direction = "alternate"
iterations = 3

# Optional spin-off sub-section
[profile.clean.spinoff]
lift_mm = 20        # mm to lift above jar (automated only)
rpm = 150           # fast spin to shed solution
time_s = 10         # duration of spin-off
```

⸻

Spin-off fields

Field       Type     Required  Description                      Constraints
────────────────────────────────────────────────────────────────────────────
lift_mm     integer  yes       Height above jar for spin-off    5–50 mm
rpm         integer  yes       Spin speed during spin-off       60–200 RPM
time_s      integer  yes       Spin-off duration                5–30 s

If the `[profile.*.spinoff]` section is absent, no spin-off is performed.

⸻

State machine integration

Two new states are added to handle spin-off:

State               Description
────────────────────────────────────────────────────────────────────────────
SpinOff             Basket lifted, spinning to shed solution
AwaitingSpinOff     Waiting for user to lift basket (manual machines only)

Updated transition table entries:

Current State       Event               Action                      Next State
────────────────────────────────────────────────────────────────────────────
RUNNING             ProfileFinished     Check for spinoff config    SpinOff or StepComplete
AwaitingSpinOff     UserConfirm         Start spin-off              SpinOff
SpinOff             SpinOffFinished     Move to next step           StepComplete

⸻

Execution flow diagrams

Automated machine:
```
Running → [lift basket] → SpinOff → [position to next jar] → Running (next profile)
```

Manual machine:
```
Running → AwaitingSpinOff → [user lifts] → SpinOff → [user moves to next jar] → Running
```

⸻

Design principles

- Spin-off is OPTIONAL per profile — users may disable it if not needed
- Spin-off never occurs for the final step in a program (e.g., drying)
- Spin-off timing is short (5–30 s) to avoid delays
- Higher RPM is safe at this stage — basket is above solution, no fluid shock risk
- Manual machines use explicit user confirmation for safety

⸻

16. profiles.toml Format (Critical)

Purpose

profiles.toml defines user-editable motion and drying profiles. Profiles are declarative, validated at boot, and never contain executable logic.

⸻

File structure

[profile.clean]
label = "Clean"
type = "clean"
rpm = 120
time_s = 180
direction = "alternate"
iterations = 3

[profile.rinse]
label = "Rinse"
type = "rinse"
rpm = 100
time_s = 120
direction = "cw"

[profile.dry]
label = "Dry"
type = "dry"
rpm = 160
time_s = 600
direction = "cw"

temperature_c = 45


⸻

Profile fields

Field	Type	Required	Description	Constraints
label	string	yes	Display name	1–16 chars
type	enum	yes	clean / rinse / dry	fixed set
rpm	integer	yes	Target RPM	0–250
time_s	integer	yes	Total runtime	1–5400 s
direction	enum	no	cw / ccw / alternate	default: cw
iterations	integer	conditional	Alternations	required if alternate
temperature_c	integer	conditional	Drying target	30–50 °C


⸻

Type-specific rules
	•	clean / rinse:
	•	temperature_c forbidden
	•	dry:
	•	temperature_c required

Invalid combinations cause the profile to be rejected.

⸻

Validation rules (boot-time)
	•	Maximum profiles: 8
	•	Label must be unique
	•	Profiles failing validation are skipped
	•	At least one valid profile must exist

If no valid user profiles load, firmware falls back to built-in defaults.

⸻

Design principles
	•	Profiles describe what, never how
	•	No loops, conditions, or scripting
	•	All safety limits enforced in firmware
	•	Backward-compatible schema for future extension

⸻

17. config.toml Format (Critical)

Purpose

config.toml defines board- and installation-specific settings. It allows the same firmware binary to be used across different builds without recompilation.

All values are optional; omitted values fall back to firmware defaults.

⸻

File structure

[pins]
heater_out = "GPIO18"
thermistor_adc = "ADC0"

[uart]
tx = "GPIO0"
rx = "GPIO1"

[ui]
rpm_step = 10
time_step_s = 30
temp_step_c = 5


⸻

Pin mapping ([pins])

Field	Description
heater_out	Heater MOSFET / SSR control GPIO
thermistor_adc	ADC channel for thermistor

Notes:
	•	Pin names must match RP2040 GPIO or ADC identifiers
	•	Motor STEP/DIR/ENABLE pins are not configurable
	•	Encoder is handled by V0 Display, not SKR Pico

⸻

Display UART configuration ([uart])

Field	Description	Default
tx	UART TX pin (Pico → Display)	GPIO0
rx	UART RX pin (Display → Pico)	GPIO1

The V0 Display connects via its 3-pin GPIO header to these UART pins.


⸻

UI behavior ([ui])

Field	Description	Allowed values
rpm_step	RPM adjustment increment	5 or 10
time_step_s	Time adjustment step	15 / 30 / 60
temp_step_c	Temperature increment	5


⸻

Validation rules (boot-time)
	•	Unknown keys are ignored
	•	Invalid values fall back to defaults
	•	Invalid pin identifiers cause the config file to be ignored

Firmware must never fail to boot due to config.toml errors.

⸻

Design principles
	•	config.toml configures wiring and ergonomics, not behavior
	•	No timing-critical or safety-critical values exposed
	•	Safe defaults always exist

⸻

18. V0 Display Communication Protocol (Critical)

Purpose

Define the UART-based communication protocol between the SKR Pico (main controller)
and the V0 Display (UI terminal). The protocol is designed for simplicity, low
latency, and robustness.

The V0 Display acts as a "dumb terminal" — it handles only input capture and
screen rendering. All UI logic remains on the SKR Pico.

⸻

Architecture Overview

┌─────────────────────────────────────────────────────────────┐
│                        SKR Pico (RP2040)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Motor     │  │  Scheduler  │  │   State Machine     │  │
│  │  Controller │  │             │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Heater    │  │   Safety    │  │   UART Protocol     │  │
│  │  Controller │  │   Monitor   │  │   (to V0 Display)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │ UART
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   V0 Display (STM32F042)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   SH1106    │  │   Encoder   │  │   UART Protocol     │  │
│  │   Driver    │  │   Handler   │  │   (to SKR Pico)     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘

⸻

Physical Layer

Parameter         Value
──────────────────────────────────
Baud rate         115200
Data bits         8
Parity            None
Stop bits         1
Logic level       3.3 V
Connection        3-wire (TX, RX, GND)

The V0 Display's 3-pin GPIO header is used for UART.

⸻

Message Framing

All messages use a simple binary frame format:

┌───────┬────────┬──────┬─────────────┬──────────┐
│ START │ LENGTH │ TYPE │ PAYLOAD     │ CHECKSUM │
│ 1B    │ 1B     │ 1B   │ 0–250B      │ 1B       │
└───────┴────────┴──────┴─────────────┴──────────┘

Field       Size    Description
─────────────────────────────────────────────────────────
START       1       Fixed: 0xAA (synchronization byte)
LENGTH      1       Payload length (0–250)
TYPE        1       Message type identifier
PAYLOAD     0–250   Type-specific data
CHECKSUM    1       XOR of LENGTH, TYPE, and all PAYLOAD bytes

Maximum frame size: 254 bytes

⸻

Message Types: Display → Pico

Type    ID      Payload             Description
────────────────────────────────────────────────────────────────
INPUT   0x01    [event: u8]         User input event
PING    0x02    (none)              Heartbeat request
ACK     0x03    [seq: u8]           Acknowledge received command

Input event values:

Event               Value   Description
────────────────────────────────────────────────────────────────
ENCODER_CW          0x01    Encoder rotated clockwise (1 detent)
ENCODER_CCW         0x02    Encoder rotated counter-clockwise
ENCODER_CLICK       0x10    Short press (<500 ms)
ENCODER_LONG_PRESS  0x11    Long press (≥500 ms)
ENCODER_RELEASE     0x12    Button released (after long press)

The display sends one INPUT message per event. Encoder rotation events are
sent per-detent, not accumulated.

⸻

Message Types: Pico → Display

Type    ID      Payload                     Description
────────────────────────────────────────────────────────────────
CLEAR   0x20    (none)                      Clear entire screen
TEXT    0x21    [row][col][len][chars...]   Draw text at position
INVERT  0x22    [row][start_col][end_col]   Invert region (highlight)
HLINE   0x23    [row][start_col][end_col]   Draw horizontal line
PONG    0x24    (none)                      Heartbeat response
RESET   0x2F    (none)                      Reset display to boot state

⸻

TEXT Command Detail

The TEXT command draws a string at a character grid position.

Byte    Field       Description
────────────────────────────────────────────────────────────────
0       row         Row (0–7 for 8 text rows on 128x64)
1       col         Column (0–20 for 21 chars at 6px font)
2       len         String length (1–21)
3+      chars       ASCII characters (not null-terminated)

Font: 6x8 fixed-width (built into display firmware)
Grid: 21 columns × 8 rows

Special characters:
  0x01 = ▶ (play/selected indicator)
  0x02 = ⏸ (pause indicator)
  0x03 = ● (bullet)
  0x04 = ↑ (up arrow)
  0x05 = ↓ (down arrow)

⸻

INVERT Command Detail

Inverts pixel colors in a rectangular region. Used for menu selection highlight.

Byte    Field       Description
────────────────────────────────────────────────────────────────
0       row         Row to invert (0–7)
1       start_col   Starting column (0–20)
2       end_col     Ending column (0–20), inclusive

To highlight a full row: INVERT(row, 0, 20)

⸻

Screen Update Pattern

The Pico constructs screens by sending command sequences:

Example: Render menu screen

  CLEAR
  TEXT(0, 0, "▶ Clean")
  INVERT(0, 0, 20)
  TEXT(1, 0, "  Rinse")
  TEXT(2, 0, "  Dry")

Example: Render runtime screen

  CLEAR
  TEXT(0, 0, "Running: Clean")
  TEXT(2, 0, "RPM:  120")
  TEXT(3, 0, "Time: 1:42")
  TEXT(5, 0, "Direction: CW")
  TEXT(7, 0, "▶ PAUSE")
  INVERT(7, 0, 20)

The display does not buffer commands — each command renders immediately.
The Pico should send CLEAR before redrawing to avoid artifacts.

⸻

Timing and Latency

Constraint                      Value
────────────────────────────────────────────────────────────────
Max input latency               10 ms (encoder event to TX)
Screen update budget            50 ms (full screen redraw)
Heartbeat interval              1000 ms
Heartbeat timeout               3000 ms (3 missed = link failure)

⸻

Error Handling

Link failure is a safety-critical event. If communication is lost, the user
has no way to pause or stop the machine.

Pico-side behavior (authoritative)

Condition                       Action
────────────────────────────────────────────────────────────────
Missed heartbeat (PING)         Increment miss counter
3 consecutive misses            Retry sequence (see below)
Retry sequence fails            Transition to ERROR state
                                Motor stopped, heater disabled

Retry sequence:
  1. Send PONG (in case display missed it)
  2. Wait 500 ms for PING
  3. Repeat up to 3 times
  4. If no valid PING received → link failure confirmed

On link failure:
  • Motor decelerates to stop (controlled, not instant disable)
  • Heater disabled immediately
  • State machine transitions to ERROR
  • Error reason stored: LINK_LOST

Recovery:
  • Link failure requires power cycle to clear
  • This is intentional — ensures user inspects the machine

Display-side behavior (informational)

Condition                       Action
────────────────────────────────────────────────────────────────
Missed PONG (3 consecutive)     Show "LINK LOST" on screen
                                Continue sending PING (for recovery)

The display cannot stop the machine — it only shows status.
The Pico is solely responsible for safety actions.

Frame-level errors

Condition                       Behavior
────────────────────────────────────────────────────────────────
Invalid checksum                Frame discarded, no retry
Unknown message type            Frame discarded silently
Incomplete frame (timeout)      Discard buffer, resync on next START

Frame errors do NOT count toward link failure. Only missed heartbeats
indicate a true communication breakdown.

⸻

Initialization Sequence

On power-up or RESET:

1. Display boots, shows "Connecting..." on screen
2. Display sends PING
3. Pico responds with PONG
4. Pico sends CLEAR + initial screen content
5. Display enters normal operation

If no PONG received within 3000 ms, display shows "NO CONTROLLER" error
and retries indefinitely.

⸻

Design Principles

• Display has no knowledge of profiles, state machine, or application logic
• Display only knows how to: render text, highlight regions, handle encoder
• All UI decisions (what to show, how to respond) live on the Pico
• Protocol is stateless — each command is self-contained
• No queuing — commands execute immediately
• Link failure always stops the machine
• The Pico never assumes the link is healthy without proof (heartbeat)
• Safety actions are never delegated to the display
• Recovery from link failure requires user intervention (power cycle)

⸻

19. Stretch Goals (Out of Scope for v1)

• Neopixel status indicator (V0 Display onboard LED)
  - Green = IDLE
  - Blue = RUNNING
  - Red = ERROR
• Heated cleaning jars
• Active airflow control
• Lid interlock
• Networked UI / remote monitoring
• Logging and statistics

⸻

