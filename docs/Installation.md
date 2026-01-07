# Installation Guide

This guide walks you through installing and deploying the Isochron firmware to your watch cleaning machine.

## Overview

Installing Isochron involves three steps:

1. **Install build tools** - Rust toolchain and flashing utilities
2. **Build the firmware** - Compile for your board
3. **Flash to board** - Deploy firmware to your controller

## Requirements

### Hardware

- **Controller board**: SKR Pico (or compatible RP2040 board)
- **USB cable**: For programming and power
- **Debug probe** (recommended): For RTT logging and faster development
  - Raspberry Pi Debug Probe (~$12) - Recommended
  - Second Pico running picoprobe - Free alternative
  - See [Developer Guide](DEVELOPER.md) for probe setup details

### Software

- **Operating System**: Linux, macOS, or Windows with WSL2
- **Rust**: 1.75 or later
- **Git**: For cloning the repository

---

## Step 1: Install Build Tools

### Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts, then restart your terminal or run:

```bash
source $HOME/.cargo/env
```

### Add Embedded Target

Install the ARM Cortex-M0+ target (used by RP2040):

```bash
rustup target add thumbv6m-none-eabi
```

### Install probe-rs (Recommended)

probe-rs provides flashing and debugging capabilities:

```bash
cargo install probe-rs-tools
```

Verify installation:

```bash
probe-rs info
```

### Install elf2uf2-rs (Alternative - No Probe)

If you don't have a debug probe, install the UF2 converter:

```bash
cargo install elf2uf2-rs
```

---

## Step 2: Get the Source Code

Clone the repository:

```bash
git clone https://github.com/your-org/chronohub.git
cd chronohub/cleaner
```

---

## Step 3: Build the Firmware

Navigate to the firmware directory:

```bash
cd isochron-firmware
```

### Build for SKR Pico (Default)

```bash
# Debug build (faster compile, larger binary, debug symbols)
cargo build

# Release build (optimized, smaller binary)
cargo build --release
```

### Verify Build

Check that the binary fits in flash (RP2040 has 2MB):

```bash
cargo size --release
```

Expected output shows text/data/bss sections, total should be under 2MB.

---

## Step 4: Flash the Firmware

### Option A: Using Debug Probe (Recommended)

Connect your debug probe to the SKR Pico's SWD header:

| Probe Pin | SKR Pico |
|-----------|----------|
| SWCLK | SWCLK |
| SWDIO | SWDIO |
| GND | GND |

Power the SKR Pico via USB or 24V, then flash:

```bash
# Flash and run with log output
cargo run --release

# Or just flash without running
probe-rs download --chip RP2040 target/thumbv6m-none-eabi/release/isochron-firmware
```

You'll see RTT log output in your terminal showing the firmware starting up.

### Option B: Using UF2 Bootloader (No Probe)

If you don't have a debug probe:

1. **Build the firmware**:
   ```bash
   cargo build --release
   ```

2. **Convert to UF2**:
   ```bash
   elf2uf2-rs target/thumbv6m-none-eabi/release/isochron-firmware isochron.uf2
   ```

3. **Enter bootloader mode**:
   - Disconnect the SKR Pico from power
   - Hold the BOOTSEL button on the RP2040
   - Connect USB cable while holding BOOTSEL
   - Release BOOTSEL - a drive named `RPI-RP2` appears

4. **Copy the firmware**:
   - Drag `isochron.uf2` to the `RPI-RP2` drive
   - The board automatically reboots and runs the firmware

**Note:** UF2 method doesn't provide RTT logging. Consider getting a debug probe for development.

---

## Step 5: Connect the Display

The firmware communicates with a V0 Display via UART.

### Wiring

Connect the V0 Display to the SKR Pico's UART header:

| SKR Pico | V0 Display |
|----------|------------|
| GPIO0 (TX) | RX |
| GPIO1 (RX) | TX |
| GND | GND |
| 5V | 5V |

The SKR Pico has a dedicated 4-pin UART connector that matches this pinout.

### Verify Connection

After flashing, the display should show the boot screen, then the program menu.

---

## Step 6: Configure Your Machine

The firmware comes with a default configuration for a basic manual cleaning machine. To customize for your specific setup, see the [Configuration Reference](Config_Reference.md).

### Default Configuration

The default config provides:

- **Spin motor** on Stepper X slot (GPIO 10, 11, 12)
- **Dryer heater** on HE0 (GPIO 23) with thermistor on TH0 (GPIO 27)
- **Three jars**: clean, rinse, dry
- **Three profiles**: Clean (3 min), Rinse (2 min), Dry (10 min)
- **Two programs**: Full Clean, Quick Clean

### Creating a Custom Config

1. Create a `machine.toml` file based on the reference
2. Flash it to the config partition (see Config Reference)
3. Restart the firmware

---

## Troubleshooting

### Build Fails with "target not found"

Ensure the embedded target is installed:

```bash
rustup target add thumbv6m-none-eabi
```

### "No probe found" Error

- Check USB connection to the debug probe
- Run `probe-rs list` to see detected probes
- Try a different USB port
- See [Developer Guide](DEVELOPER.md) for probe troubleshooting

### UF2 Drive Doesn't Appear

- Ensure you're holding BOOTSEL before connecting USB
- Try a different USB cable (some cables are charge-only)
- Check if the RP2040 chip is working (LED activity)

### Firmware Runs But Display is Blank

- Check UART wiring (TX→RX, RX→TX)
- Verify 5V power to the display
- Check baud rate matches (default: 115200)
- Verify display firmware is correct version

### Stepper Motor Doesn't Move

- Check motor wiring to stepper driver slot
- Verify ENABLE pin is being driven (active low on SKR Pico)
- Check TMC2209 UART address matches config
- See RTT logs for error messages

---

## Next Steps

- [Configuration Reference](Config_Reference.md) - Customize your machine
- [Supported Boards](Boards.md) - Hardware options
- [Developer Guide](DEVELOPER.md) - For development and debugging
- [Architecture](ARCHITECTURE.md) - Understand the firmware design
