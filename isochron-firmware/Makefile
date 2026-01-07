# Isochron Firmware Build System
#
# Usage:
#   make          - Build release firmware and generate UF2
#   make flash    - Build and flash via debug probe
#   make uf2      - Build and generate UF2 only
#   make clean    - Clean build artifacts
#
# Prerequisites:
#   cargo install elf2uf2-rs   # For UF2 generation
#   cargo install probe-rs-tools  # For debug probe flashing

.PHONY: all build uf2 flash clean install-tools help

# Output paths
TARGET_DIR := ../target/thumbv6m-none-eabi/release
ELF := $(TARGET_DIR)/isochron-firmware
UF2 := $(TARGET_DIR)/isochron-firmware.uf2
UF2_COPY := isochron-firmware.uf2

# Default target: build and create UF2
all: uf2
	@echo ""
	@echo "✓ Build complete!"
	@echo "  UF2 file: $(UF2_COPY)"
	@echo ""
	@echo "To flash:"
	@echo "  1. Hold BOOTSEL button and connect USB"
	@echo "  2. Copy $(UF2_COPY) to RPI-RP2 drive"

# Build release firmware
build:
	@echo "Building firmware..."
	cargo build --release

# Build and generate UF2 file
uf2: build
	@echo "Generating UF2..."
	@elf2uf2-rs $(ELF) $(UF2) 2>/dev/null || \
		(echo "Error: elf2uf2-rs not found. Install with: cargo install elf2uf2-rs" && exit 1)
	@cp $(UF2) $(UF2_COPY)
	@echo "Generated: $(UF2_COPY) ($$(du -h $(UF2_COPY) | cut -f1))"

# Flash via debug probe (probe-rs)
flash: build
	@echo "Flashing via debug probe..."
	@probe-rs run --chip RP2040 $(ELF) 2>/dev/null || \
		(echo "Error: probe-rs not found or no probe connected." && \
		 echo "Install with: cargo install probe-rs-tools" && exit 1)

# Flash UF2 to mounted RPI-RP2 drive (macOS)
flash-uf2: uf2
	@if [ -d "/Volumes/RPI-RP2" ]; then \
		echo "Copying to RPI-RP2..."; \
		cp $(UF2_COPY) /Volumes/RPI-RP2/; \
		echo "✓ Flashed! Board will reboot automatically."; \
	else \
		echo "Error: RPI-RP2 drive not found."; \
		echo "Hold BOOTSEL button and connect USB to enter bootloader mode."; \
		exit 1; \
	fi

# Clean build artifacts
clean:
	cargo clean
	rm -f $(UF2_COPY)

# Install required tools
install-tools:
	@echo "Installing build tools..."
	cargo install elf2uf2-rs
	cargo install probe-rs-tools
	@echo "✓ Tools installed"

# Help
help:
	@echo "Isochron Firmware Build System"
	@echo ""
	@echo "Targets:"
	@echo "  make           Build and generate UF2 (default)"
	@echo "  make build     Build release firmware only"
	@echo "  make uf2       Build and generate UF2"
	@echo "  make flash     Build and flash via debug probe"
	@echo "  make flash-uf2 Build UF2 and copy to RPI-RP2 drive"
	@echo "  make clean     Clean build artifacts"
	@echo "  make install-tools  Install elf2uf2-rs and probe-rs"
	@echo ""
	@echo "Configuration:"
	@echo "  Edit machine.toml to customize your machine"
	@echo "  See docs/Config_Reference.md for options"
