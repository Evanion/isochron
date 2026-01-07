//! Hardware configuration types
//!
//! These types define the hardware-level configuration for pins,
//! stepper drivers, heaters, and other peripherals.

use heapless::{String, Vec};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::types::{
    HeaterConfig, JarConfig, ProfileConfig, ProgramConfig, UiConfig,
    MAX_JARS, MAX_LABEL_LEN, MAX_PROFILES, MAX_PROGRAMS,
};

/// Maximum steppers per config
pub const MAX_STEPPERS: usize = 4;

/// Maximum heaters per config  
pub const MAX_HEATERS: usize = 4;

/// Pin configuration with optional inversion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PinConfig {
    /// GPIO pin number (0-29 for RP2040)
    pub pin: u8,
    /// Pin is active-low (inverted)
    pub inverted: bool,
    /// Enable internal pull-up
    pub pull_up: bool,
}

impl PinConfig {
    /// Create a new pin config
    pub const fn new(pin: u8) -> Self {
        Self {
            pin,
            inverted: false,
            pull_up: false,
        }
    }

    /// Create an inverted (active-low) pin
    pub const fn inverted(pin: u8) -> Self {
        Self {
            pin,
            inverted: true,
            pull_up: false,
        }
    }

    /// Create a pin with pull-up enabled
    pub const fn with_pullup(pin: u8) -> Self {
        Self {
            pin,
            inverted: false,
            pull_up: true,
        }
    }
}

/// Stepper motor hardware configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StepperHwConfig {
    /// Stepper name (e.g., "spin", "lift", "tower")
    pub name: String<MAX_LABEL_LEN>,
    /// Step pulse pin
    pub step_pin: PinConfig,
    /// Direction pin
    pub dir_pin: PinConfig,
    /// Enable pin (active-low typically)
    pub enable_pin: PinConfig,
    /// Endstop pin (optional, for position-controlled steppers)
    pub endstop_pin: Option<PinConfig>,
    /// Full steps per motor rotation (typically 200 for 1.8° motors)
    pub full_steps_per_rotation: u16,
    /// Microsteps setting
    pub microsteps: u8,
    /// Rotation distance (mm for linear, degrees for rotational)
    pub rotation_distance: u16,
    /// Gear ratio numerator (e.g., 3 for 3:1)
    pub gear_ratio_num: u8,
    /// Gear ratio denominator (e.g., 1 for 3:1)
    pub gear_ratio_den: u8,
}

/// TMC2209 driver configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Tmc2209HwConfig {
    /// Name of the associated stepper
    pub stepper_name: String<MAX_LABEL_LEN>,
    /// UART TX pin
    pub uart_tx_pin: u8,
    /// UART RX pin
    pub uart_rx_pin: u8,
    /// UART address (0-3 for multi-driver bus)
    pub uart_address: u8,
    /// Run current in mA
    pub run_current_ma: u16,
    /// Hold current in mA
    pub hold_current_ma: u16,
    /// StallGuard threshold (for sensorless homing)
    pub stall_threshold: u8,
    /// Enable StealthChop mode
    pub stealthchop: bool,
    /// DIAG pin for StallGuard (optional)
    pub diag_pin: Option<u8>,
}

/// Heater hardware configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HeaterHwConfig {
    /// Heater name (matches HeaterConfig.name)
    pub name: String<MAX_LABEL_LEN>,
    /// Heater output pin (GPIO for SSR/MOSFET)
    pub heater_pin: PinConfig,
    /// Temperature sensor ADC pin
    pub sensor_pin: u8,
    /// Sensor type
    pub sensor_type: SensorType,
}

/// Temperature sensor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SensorType {
    /// NTC 100K thermistor (typical 3D printer type)
    #[default]
    Ntc100k,
    /// NTC 10K thermistor
    Ntc10k,
    /// PT100 RTD (future)
    Pt100,
}

/// Display configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DisplayHwConfig {
    /// UART TX pin
    pub uart_tx_pin: u8,
    /// UART RX pin
    pub uart_rx_pin: u8,
    /// Baud rate
    pub baud_rate: u32,
}

/// Complete machine configuration
///
/// This is the top-level configuration structure that contains all
/// hardware and profile configuration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MachineConfig {
    /// Configuration version for compatibility checks
    pub version: u8,
    /// Stepper motor configurations
    pub steppers: Vec<StepperHwConfig, MAX_STEPPERS>,
    /// TMC2209 driver configurations
    pub tmc2209s: Vec<Tmc2209HwConfig, MAX_STEPPERS>,
    /// Heater hardware configurations
    pub heater_hw: Vec<HeaterHwConfig, MAX_HEATERS>,
    /// Heater control configurations
    pub heaters: Vec<HeaterConfig, MAX_HEATERS>,
    /// Jar configurations
    pub jars: Vec<JarConfig, MAX_JARS>,
    /// Profile configurations
    pub profiles: Vec<ProfileConfig, MAX_PROFILES>,
    /// Program configurations
    pub programs: Vec<ProgramConfig, MAX_PROGRAMS>,
    /// Display configuration
    pub display: DisplayHwConfig,
    /// UI configuration
    pub ui: UiConfig,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            version: 1,
            steppers: Vec::new(),
            tmc2209s: Vec::new(),
            heater_hw: Vec::new(),
            heaters: Vec::new(),
            jars: Vec::new(),
            profiles: Vec::new(),
            programs: Vec::new(),
            display: DisplayHwConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl MachineConfig {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a stepper by name
    pub fn find_stepper(&self, name: &str) -> Option<&StepperHwConfig> {
        self.steppers.iter().find(|s| s.name.as_str() == name)
    }

    /// Find a heater by name
    pub fn find_heater(&self, name: &str) -> Option<&HeaterConfig> {
        self.heaters.iter().find(|h| h.name.as_str() == name)
    }

    /// Find a jar by name
    pub fn find_jar(&self, name: &str) -> Option<&JarConfig> {
        self.jars.iter().find(|j| j.name.as_str() == name)
    }

    /// Find a profile by name
    pub fn find_profile(&self, name: &str) -> Option<&ProfileConfig> {
        self.profiles.iter().find(|p| p.label.as_str() == name)
    }

    /// Find a program by name
    pub fn find_program(&self, name: &str) -> Option<&ProgramConfig> {
        self.programs.iter().find(|p| p.label.as_str() == name)
    }

    /// Check if this is an automated machine (has lift and tower steppers)
    pub fn is_automated(&self) -> bool {
        self.find_stepper("lift").is_some() && self.find_stepper("tower").is_some()
    }

    /// Get the spin stepper (required)
    pub fn spin_stepper(&self) -> Option<&StepperHwConfig> {
        self.find_stepper("spin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_config() {
        let pin = PinConfig::new(10);
        assert_eq!(pin.pin, 10);
        assert!(!pin.inverted);
        assert!(!pin.pull_up);

        let inverted = PinConfig::inverted(12);
        assert!(inverted.inverted);

        let pullup = PinConfig::with_pullup(4);
        assert!(pullup.pull_up);
    }

    #[test]
    fn test_empty_config() {
        let config = MachineConfig::new();
        assert!(config.steppers.is_empty());
        assert!(!config.is_automated());
        assert!(config.spin_stepper().is_none());
    }
}
