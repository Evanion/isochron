# Isochron Firmware Makefile
#
# Klipper-style build system using kconfiglib for configuration.
#
# Usage:
#   make menuconfig      - Configure build options
#   make build           - Build firmware
#   make flash           - Flash firmware to device
#   make clean           - Clean build artifacts
#
#   make list-profiles   - List available profiles
#   make profile PROFILE=<name>  - Load a profile
#   make save-profile PROFILE=<name>  - Save current config as profile

# Directories
OUT_DIR := out
PROFILE_DIR := profiles
SHIPPED_PROFILES := $(PROFILE_DIR)/shipped
USER_PROFILES := $(PROFILE_DIR)/user

# Tools
CARGO := cargo
MENUCONFIG := menuconfig
ELF2UF2 := elf2uf2-rs
PROBE_RS := probe-rs

# Targets
RP2040_TARGET := thumbv6m-none-eabi
STM32F0_TARGET := thumbv6m-none-eabi

# Default goal
.DEFAULT_GOAL := help

# Include .config if it exists
-include .config

.PHONY: help menuconfig build flash clean list-profiles profile save-profile check

help:
	@echo "Isochron Firmware Build System"
	@echo ""
	@echo "Configuration:"
	@echo "  make menuconfig     - Configure build options (like Klipper)"
	@echo "  make defconfig      - Create default .config"
	@echo ""
	@echo "Building:"
	@echo "  make build          - Build firmware based on .config"
	@echo "  make check          - Check firmware compiles without building"
	@echo "  make clean          - Remove build artifacts"
	@echo ""
	@echo "Flashing:"
	@echo "  make flash          - Flash controller firmware"
	@echo "  make flash-display  - Flash display firmware (if applicable)"
	@echo ""
	@echo "Profiles:"
	@echo "  make list-profiles              - Show available profiles"
	@echo "  make profile PROFILE=<name>     - Load a saved profile"
	@echo "  make save-profile PROFILE=<name> - Save current config as profile"
	@echo ""
	@echo "Prerequisites:"
	@echo "  pip install kconfiglib    # For menuconfig"
	@echo "  cargo install elf2uf2-rs  # For UF2 generation"
	@echo "  cargo install probe-rs    # For flashing (optional)"

# Configuration
menuconfig: Kconfig
	@command -v $(MENUCONFIG) >/dev/null 2>&1 || { \
		echo "Error: kconfiglib not installed. Run: pip install kconfiglib"; \
		exit 1; \
	}
	$(MENUCONFIG) Kconfig
	@echo ""
	@echo "Configuration saved to .config"
	@echo "Run 'make build' to build firmware"

defconfig:
	@echo "Creating default configuration..."
	@echo "# Isochron default configuration" > .config
	@echo "CONFIG_BOARD_BTT_PICO=y" >> .config
	@echo "CONFIG_MOTOR_STEPPER=y" >> .config
	@echo "CONFIG_MOTOR_COUNT=3" >> .config
	@echo "CONFIG_TMC_UART_ENABLED=y" >> .config
	@echo "CONFIG_TMC_DEFAULT_CURRENT_MA=800" >> .config
	@echo "CONFIG_TMC_DEFAULT_MICROSTEPS=16" >> .config
	@echo "CONFIG_TMC_STEALTHCHOP=y" >> .config
	@echo "CONFIG_DISPLAY_V0_MINI=y" >> .config
	@echo "CONFIG_BUILD_DISPLAY_FW=y" >> .config
	@echo "CONFIG_DISPLAY_UART_BAUD=115200" >> .config
	@echo "CONFIG_MACHINE_NAME=\"Isochron Cleaner\"" >> .config
	@echo "CONFIG_FEATURE_USB_SERIAL=y" >> .config
	@echo "CONFIG_FEATURE_WATCHDOG=y" >> .config
	@echo "CONFIG_FEATURE_FLASH_CONFIG=y" >> .config
	@echo "CONFIG_DEFMT_LOG=y" >> .config
	@echo "CONFIG_LOG_LEVEL_INFO=y" >> .config
	@echo "Default configuration saved to .config"

# Ensure .config exists
.config:
	@echo "No .config found. Run 'make menuconfig' or 'make defconfig' first."
	@exit 1

# Build
build: .config
	@echo "Building Isochron firmware..."
	@mkdir -p $(OUT_DIR)
	$(eval include .config)
	@# Determine build profile
	$(eval BUILD_PROFILE := $(if $(CONFIG_RELEASE_BUILD),release,dev))
	$(eval CARGO_FLAGS := $(if $(CONFIG_RELEASE_BUILD),--release,))
	@# Build controller firmware
	@echo "Building controller firmware ($(BUILD_PROFILE))..."
	$(CARGO) build -p isochron-firmware $(CARGO_FLAGS) --target $(RP2040_TARGET)
	@# Generate UF2
	@command -v $(ELF2UF2) >/dev/null 2>&1 && { \
		$(ELF2UF2) target/$(RP2040_TARGET)/$(BUILD_PROFILE)/isochron-firmware $(OUT_DIR)/isochron-firmware.uf2; \
		echo "Generated $(OUT_DIR)/isochron-firmware.uf2"; \
	} || { \
		echo "Warning: elf2uf2-rs not installed. Skipping UF2 generation."; \
		echo "Install with: cargo install elf2uf2-rs"; \
	}
	@# Build display firmware if configured
ifdef CONFIG_BUILD_DISPLAY_FW
	@echo "Building display firmware..."
	$(CARGO) build -p isochron-display-fw $(CARGO_FLAGS) --target $(STM32F0_TARGET)
	@# Generate UF2 for display (STM32 uses different format, but keeping consistent)
	@command -v $(ELF2UF2) >/dev/null 2>&1 && { \
		$(ELF2UF2) target/$(STM32F0_TARGET)/$(BUILD_PROFILE)/isochron-display-fw $(OUT_DIR)/isochron-display-fw.uf2 2>/dev/null || \
		echo "Note: Display firmware is ELF format (flash via probe-rs or STM32CubeProgrammer)"; \
		cp target/$(STM32F0_TARGET)/$(BUILD_PROFILE)/isochron-display-fw $(OUT_DIR)/isochron-display-fw.elf; \
	}
	@echo "Generated $(OUT_DIR)/isochron-display-fw.elf"
endif
	@echo ""
	@echo "Build complete!"
	@echo "Firmware: $(OUT_DIR)/isochron-firmware.uf2"

check: .config
	@echo "Checking firmware compiles..."
	$(CARGO) check -p isochron-firmware --target $(RP2040_TARGET)
	@echo "Check passed!"

clean:
	$(CARGO) clean
	rm -rf $(OUT_DIR)
	@echo "Clean complete"

# Flashing
flash: build
	@echo "Flashing controller firmware..."
	@command -v $(PROBE_RS) >/dev/null 2>&1 && { \
		$(PROBE_RS) run --chip RP2040 $(OUT_DIR)/isochron-firmware.uf2; \
	} || { \
		echo "probe-rs not installed. Flash manually:"; \
		echo "1. Hold BOOTSEL button on board"; \
		echo "2. Connect USB"; \
		echo "3. Copy $(OUT_DIR)/isochron-firmware.uf2 to the RPI-RP2 drive"; \
	}

flash-display: build
	@echo "Flashing display firmware..."
	@if [ ! -f "$(OUT_DIR)/isochron-display-fw.elf" ]; then \
		echo "Error: Display firmware not built. Enable BUILD_DISPLAY_FW in menuconfig."; \
		exit 1; \
	fi
	@command -v $(PROBE_RS) >/dev/null 2>&1 && { \
		$(PROBE_RS) run --chip STM32F042K6Tx $(OUT_DIR)/isochron-display-fw.elf; \
	} || { \
		echo "probe-rs not installed. Flash manually with STM32CubeProgrammer:"; \
		echo "1. Connect ST-Link or USB DFU"; \
		echo "2. Flash $(OUT_DIR)/isochron-display-fw.elf"; \
	}

# Profiles
list-profiles:
	@echo "Available profiles:"
	@echo ""
	@echo "Shipped profiles:"
	@if [ -d "$(SHIPPED_PROFILES)" ]; then \
		for f in $(SHIPPED_PROFILES)/*.config; do \
			if [ -f "$$f" ]; then \
				name=$$(basename "$$f" .config); \
				desc=$$(grep "^# Description:" "$$f" 2>/dev/null | cut -d: -f2- | xargs); \
				printf "  %-20s %s\n" "$$name" "$$desc"; \
			fi; \
		done; \
	else \
		echo "  (none)"; \
	fi
	@echo ""
	@echo "User profiles:"
	@if [ -d "$(USER_PROFILES)" ]; then \
		for f in $(USER_PROFILES)/*.config; do \
			if [ -f "$$f" ]; then \
				name=$$(basename "$$f" .config); \
				date=$$(stat -f "%Sm" -t "%Y-%m-%d" "$$f" 2>/dev/null || stat -c "%y" "$$f" 2>/dev/null | cut -d' ' -f1); \
				printf "  %-20s (saved %s)\n" "$$name" "$$date"; \
			fi; \
		done; \
	else \
		echo "  (none)"; \
	fi
	@echo ""
	@echo "Load a profile with: make profile PROFILE=<name>"

profile:
ifndef PROFILE
	@echo "Error: specify profile name with PROFILE=<name>"
	@echo "Run 'make list-profiles' to see available profiles"
	@exit 1
endif
	@# Check shipped profiles first, then user profiles
	@if [ -f "$(SHIPPED_PROFILES)/$(PROFILE).config" ]; then \
		cp "$(SHIPPED_PROFILES)/$(PROFILE).config" .config; \
		echo "Loaded shipped profile: $(PROFILE)"; \
	elif [ -f "$(USER_PROFILES)/$(PROFILE).config" ]; then \
		cp "$(USER_PROFILES)/$(PROFILE).config" .config; \
		echo "Loaded user profile: $(PROFILE)"; \
	else \
		echo "Error: profile '$(PROFILE)' not found"; \
		echo "Run 'make list-profiles' to see available profiles"; \
		exit 1; \
	fi

save-profile:
ifndef PROFILE
	@echo "Error: specify profile name with PROFILE=<name>"
	@exit 1
endif
	@if [ ! -f ".config" ]; then \
		echo "Error: no .config to save. Run 'make menuconfig' first."; \
		exit 1; \
	fi
	@mkdir -p $(USER_PROFILES)
	@cp .config "$(USER_PROFILES)/$(PROFILE).config"
	@echo "Saved current config as user profile: $(PROFILE)"

# Development helpers
.PHONY: test doc

test:
	$(CARGO) test --workspace --exclude isochron-firmware

doc:
	$(CARGO) doc --workspace --no-deps
	@echo "Documentation generated in target/doc/"
