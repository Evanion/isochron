# isochron-firmware

> **Warning**
> This is **alpha software** under active development. APIs, configuration formats, and behavior may change without notice. Not recommended for production use.

Embedded firmware for the Isochron watch cleaning machine, targeting RP2040-based boards (SKR Pico).

## Prerequisites

### Hardware
- SKR Pico board (or compatible RP2040 board)
- Debug probe (Raspberry Pi Debug Probe, J-Link, etc.)
- USB cable

### Software
```bash
# Install Rust embedded target
rustup target add thumbv6m-none-eabi

# Install probe-rs tools
cargo install probe-rs-tools

# Verify probe-rs installation
probe-rs info
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized for size)
cargo build --release

# Check binary size
cargo size --release
```

## Flashing & Running

### With Debug Probe (Recommended)

Connect your debug probe to the SKR Pico's SWD header (SWCLK, SWDIO, GND).

```bash
# Flash and run with RTT output (debug build)
cargo run

# Flash and run release build
cargo run --release

# Using aliases
cargo rb      # Run debug build
cargo rrb     # Run release build
```

### UF2 Bootloader (No Probe)

If you don't have a debug probe:

```bash
# Build UF2 file
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/isochron-firmware isochron.uf2

# Then:
# 1. Hold BOOTSEL button on Pico while connecting USB
# 2. Copy isochron.uf2 to the RPI-RP2 drive
```

Install elf2uf2-rs: `cargo install elf2uf2-rs`

## Debugging

### RTT Logging

Log output uses defmt over RTT. Set log level in `.cargo/config.toml`:

```toml
[env]
DEFMT_LOG = "trace"  # trace, debug, info, warn, error
```

Log output appears in terminal when using `cargo run`.

### GDB Debugging

```bash
# Terminal 1: Start GDB server
probe-rs gdb --chip RP2040

# Terminal 2: Connect with GDB
arm-none-eabi-gdb -x gdb.init target/thumbv6m-none-eabi/debug/isochron-firmware
```

### probe-rs Profiles

Edit `Embed.toml` for probe-rs configuration:
- `[dev]` - Fast iteration (skips verify)
- `[release]` - Full verification

## Pin Assignments (SKR Pico)

| Function | GPIO | Notes |
|----------|------|-------|
| STEP | GPIO11 | Stepper X step |
| DIR | GPIO10 | Stepper X direction |
| ENABLE | GPIO12 | Active low |
| TMC TX | GPIO8 | TMC2209 UART |
| TMC RX | GPIO9 | TMC2209 UART |
| TMC DIAG | GPIO17 | StallGuard output |
| Display TX | GPIO0 | UART0 to V0 Display |
| Display RX | GPIO1 | UART0 from V0 Display |
| Heater | GPIO23 | HE0 output |
| Thermistor | GPIO27 | TH0 ADC input |

## Troubleshooting

### "No probe found"
- Check USB connection to debug probe
- Verify probe drivers are installed
- Run `probe-rs info` to list detected probes

### "Failed to attach to target"
- Check SWD wiring (SWCLK, SWDIO, GND)
- Ensure target is powered
- Try lower SWD speed: add `--speed 1000` to runner

### No RTT Output
- Verify defmt is enabled in features
- Check `DEFMT_LOG` environment variable
- Ensure RTT buffer isn't overflowing (try `NoBlockSkip` mode)

### Build Errors
- Ensure correct target: `rustup target add thumbv6m-none-eabi`
- Check workspace is building: `cargo build` from repo root

## Architecture

See `../brief.md` and plan file for detailed architecture documentation.
