# Developer Guide

This guide covers setting up a development environment for the Isochron firmware.

## Prerequisites

### Rust Toolchain

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add the embedded ARM target
rustup target add thumbv6m-none-eabi

# Install probe-rs tools
cargo install probe-rs-tools

# Verify installation
probe-rs info
```

### Hardware

- SKR Pico board (or compatible RP2040 board)
- Debug probe (see options below)
- USB cables
- SWD jumper wires (if probe doesn't have ribbon cable)

---

## Debug Probe Options

You need a debug probe to flash firmware and view RTT log output. Several options are available:

### Option 1: Raspberry Pi Debug Probe (Recommended)

**Cost:** ~$12 USD

The official Raspberry Pi Debug Probe is purpose-built for this task. It includes both SWD debugging and a UART passthrough.

**Pros:**
- Plug and play, no setup required
- Includes proper SWD cable with JST-SH connector
- Also provides UART passthrough (useful for display debugging)
- Compact form factor

**Cons:**
- Requires purchase

**Purchase:** [Raspberry Pi Foundation](https://www.raspberrypi.com/products/debug-probe/) or authorized resellers

### Option 2: Picoprobe (Another Pico as Debugger)

**Cost:** Free (if you have a spare Pico)

Flash a second Raspberry Pi Pico with picoprobe firmware to use it as a debug probe.

**Pros:**
- Free if you have a spare Pico
- Same capabilities as the official debug probe

**Cons:**
- Requires a second Pico
- Need to flash picoprobe firmware
- Manual wiring required

**Setup:**
```bash
# Download picoprobe UF2 from:
# https://github.com/raspberrypi/picoprobe/releases

# Hold BOOTSEL on the spare Pico, connect USB
# Copy picoprobe.uf2 to the RPI-RP2 drive
```

**Wiring (Picoprobe → Target SKR Pico):**
| Picoprobe Pin | Target Pin | Function |
|---------------|------------|----------|
| GP2 | SWCLK | Clock |
| GP3 | SWDIO | Data |
| GND | GND | Ground |

### Option 3: J-Link

**Cost:** $20-400+ depending on version

SEGGER J-Link is a professional debug probe with excellent performance.

**Pros:**
- Very fast flashing
- Professional-grade reliability
- Excellent software support

**Cons:**
- Expensive (EDU version ~$20, commercial ~$400+)
- Overkill for hobby projects

**Versions:**
- J-Link EDU Mini (~$20) - For educational/non-commercial use
- J-Link BASE - Commercial license

### Option 4: ST-Link (V2 or V3)

**Cost:** ~$10-25

ST-Link probes are designed for STM32 but work with RP2040 via SWD.

**Pros:**
- Inexpensive clones available
- Widely available

**Cons:**
- May need firmware update
- Clones vary in quality

### Option 5: No Probe (UF2 Bootloader)

**Cost:** Free

If you don't have a debug probe, you can still flash firmware using the built-in UF2 bootloader.

**Limitations:**
- No RTT log output (no debugging visibility)
- No breakpoint debugging
- Manual bootloader entry required each flash

**Usage:**
```bash
# Build the firmware
cargo build --release

# Convert to UF2 (install elf2uf2-rs first)
cargo install elf2uf2-rs
elf2uf2-rs target/thumbv6m-none-eabi/release/isochron-firmware isochron.uf2

# Flash:
# 1. Hold BOOTSEL button on SKR Pico
# 2. Connect USB cable
# 3. Release BOOTSEL - RPI-RP2 drive appears
# 4. Copy isochron.uf2 to the drive
```

---

## Connecting the Debug Probe

### SKR Pico SWD Header

The SKR Pico has a 3-pin SWD header. Locate it on the board (usually labeled "SWD" or "DEBUG").

```
SKR Pico SWD Header:
┌─────────────┐
│ SWCLK SWDIO │
│   O     O   │
│      O      │
│     GND     │
└─────────────┘
```

### Wiring

Connect your debug probe to the SKR Pico:

| Debug Probe | SKR Pico | Wire Color (typical) |
|-------------|----------|---------------------|
| SWCLK | SWCLK | Yellow |
| SWDIO | SWDIO | Blue |
| GND | GND | Black |

**Important:**
- Do NOT connect VCC/3.3V unless you specifically need to power the target from the probe
- The SKR Pico should be powered via its own USB or 24V input
- Double-check connections - wrong wiring can damage the board

### Verify Connection

```bash
# With probe connected and SKR Pico powered:
probe-rs info

# Expected output:
# Probing target via SWD
#
# ARM Chip:
#     Manufacturer: Raspberry Pi Trading Ltd
#     Part: RP2040
#     ...
```

If you see "No probe found", check:
- USB connection to the probe
- Probe drivers installed (usually automatic on modern OS)
- SWD wiring between probe and target

---

## Building and Flashing

### Build Commands

```bash
# Navigate to firmware directory
cd isochron-firmware

# Debug build
cargo build

# Release build (optimized for size)
cargo build --release

# Check binary size
cargo size --release
```

### Flash and Run

With a debug probe connected:

```bash
# Flash and run debug build with RTT output
cargo run

# Flash and run release build
cargo run --release

# Using aliases (defined in .cargo/config.toml)
cargo rb      # Run debug build
cargo rrb     # Run release build
```

The firmware will flash and immediately start running. RTT log output appears in the terminal.

### Flash Only (No Run)

```bash
# Flash without running
probe-rs download --chip RP2040 target/thumbv6m-none-eabi/release/isochron-firmware
```

---

## RTT Logging

RTT (Real-Time Transfer) provides printf-style debugging over the debug probe, without requiring a UART connection.

### Log Levels

Set the log level in `.cargo/config.toml`:

```toml
[env]
DEFMT_LOG = "debug"  # Options: trace, debug, info, warn, error
```

| Level | Use Case |
|-------|----------|
| `error` | Critical failures only |
| `warn` | Warnings and errors |
| `info` | General status messages (default for release) |
| `debug` | Detailed debugging info |
| `trace` | Very verbose, step-by-step logging |

### Log Macros

In firmware code, use defmt macros:

```rust
use defmt::*;

info!("System started");
debug!("Temperature: {} C", temp);
trace!("Entering function");
warn!("Unexpected value: {}", val);
error!("Critical failure!");
```

### RTT Configuration

RTT settings are in `Embed.toml`:

```toml
[default.rtt]
enabled = true
up_mode = "BlockIfFull"    # Block if buffer full (dev)
# up_mode = "NoBlockSkip"  # Skip if buffer full (release)
timeout = 3000             # RTT connection timeout (ms)
show_timestamps = true     # Show timestamps in output
```

### Viewing RTT Output

RTT output appears automatically when using `cargo run`. For standalone viewing:

```bash
# Attach to running target
probe-rs attach --chip RP2040 target/thumbv6m-none-eabi/debug/isochron-firmware
```

---

## GDB Debugging

For breakpoint debugging and memory inspection:

### Start GDB Server

```bash
# Terminal 1: Start probe-rs GDB server
probe-rs gdb --chip RP2040
```

### Connect with GDB

```bash
# Terminal 2: Connect with arm-none-eabi-gdb
arm-none-eabi-gdb target/thumbv6m-none-eabi/debug/isochron-firmware

# In GDB:
(gdb) target remote :1337
(gdb) load
(gdb) break main
(gdb) continue
```

### Common GDB Commands

```gdb
# Breakpoints
break main              # Break at function
break src/main.rs:50    # Break at line
delete 1                # Delete breakpoint 1

# Execution
continue               # Continue running
step                   # Step into
next                   # Step over
finish                 # Run until function returns

# Inspection
print variable         # Print variable value
info registers         # Show CPU registers
x/10x 0x20000000       # Examine memory (hex)
backtrace              # Show call stack

# Control
reset                  # Reset target
quit                   # Exit GDB
```

### VS Code Integration

For VS Code debugging, install the "probe-rs" extension and create `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "probe-rs-debug",
            "request": "launch",
            "name": "Debug",
            "cwd": "${workspaceFolder}/isochron-firmware",
            "chip": "RP2040",
            "flashingConfig": {
                "flashingEnabled": true,
                "resetAfterFlashing": true,
                "haltAfterReset": true
            },
            "coreConfigs": [
                {
                    "coreIndex": 0,
                    "programBinary": "${workspaceFolder}/isochron-firmware/target/thumbv6m-none-eabi/debug/isochron-firmware"
                }
            ]
        }
    ]
}
```

---

## Troubleshooting

### "No probe found"

```bash
# Check USB connection
lsusb  # Linux
system_profiler SPUSBDataType  # macOS

# List detected probes
probe-rs list
```

**Solutions:**
- Reconnect USB cable
- Try different USB port
- Install probe drivers (usually automatic)
- Check probe firmware is up to date

### "Failed to attach to target"

The probe can see the target but can't connect.

**Solutions:**
- Check SWD wiring (SWCLK, SWDIO, GND)
- Ensure target is powered
- Try lower SWD speed: `probe-rs run --speed 1000 --chip RP2040 ...`
- Check for shorts on SWD pins

### No RTT Output

The firmware runs but no log output appears.

**Solutions:**
- Verify `DEFMT_LOG` is set in `.cargo/config.toml`
- Check RTT is enabled in `Embed.toml`
- Ensure defmt feature is enabled in `Cargo.toml`
- Try increasing RTT timeout in `Embed.toml`

### Build Errors

```bash
# Ensure correct target is installed
rustup target add thumbv6m-none-eabi

# Clean and rebuild
cargo clean
cargo build
```

### "Memory region FLASH is full"

Binary too large for flash.

**Solutions:**
- Use release build: `cargo build --release`
- Enable LTO in `Cargo.toml`: `lto = true`
- Check for debug symbols in release build
- Review heap size allocation

### Firmware Crashes at Startup

**Solutions:**
- Reduce heap size if memory constrained
- Check for stack overflow (increase stack size in `memory.x`)
- Use `trace!` logging to find crash location
- Connect GDB and check backtrace

---

## Development Workflow

### Recommended Workflow

1. **Write code** - Edit in your IDE/editor
2. **Build** - `cargo build` (catches compile errors)
3. **Flash and test** - `cargo run` (see RTT output)
4. **Debug if needed** - Use RTT logs or GDB
5. **Iterate** - Repeat until working

### Fast Iteration

For faster iteration during development:

```toml
# Embed.toml - dev profile
[dev.flashing]
verify = false  # Skip verification (faster)
```

### Release Testing

Before final testing:

```bash
# Build optimized release
cargo build --release

# Check size fits in flash
cargo size --release

# Flash and test release build
cargo run --release
```

---

## Project Structure Quick Reference

```
isochron-firmware/
├── .cargo/
│   └── config.toml     # Target, runner, log level
├── Cargo.toml          # Dependencies, features
├── Embed.toml          # probe-rs configuration
├── memory.x            # Linker script (memory regions)
├── build.rs            # Build script
└── src/
    ├── main.rs         # Entry point
    ├── channels.rs     # Inter-task communication
    └── tasks/          # Embassy async tasks
```

### Key Configuration Files

| File | Purpose |
|------|---------|
| `.cargo/config.toml` | Build target, runner command, env vars |
| `Embed.toml` | probe-rs RTT/flash settings |
| `memory.x` | Flash/RAM memory regions |
| `Cargo.toml` | Dependencies, features, profiles |

---

## Useful Commands Reference

```bash
# Building
cargo build                    # Debug build
cargo build --release          # Release build
cargo size --release           # Show binary size

# Running
cargo run                      # Flash + run + RTT
cargo run --release            # Release version
cargo rb / cargo rrb           # Aliases

# Probing
probe-rs info                  # Show probe and target info
probe-rs list                  # List connected probes
probe-rs reset --chip RP2040   # Reset target

# Testing (host)
cargo test -p isochron-core    # Run core tests
cargo test -p isochron-protocol # Run protocol tests
cargo test --workspace         # Run all tests
```
