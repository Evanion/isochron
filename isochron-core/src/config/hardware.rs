//! Hardware configuration types
//!
//! These types define the hardware-level configuration for pins,
//! stepper drivers, heaters, and other peripherals.

use heapless::{String, Vec};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::types::{
    HeaterConfig, JarConfig, ProfileConfig, ProgramConfig, UiConfig, MAX_JARS, MAX_LABEL_LEN,
    MAX_PROFILES, MAX_PROGRAMS,
};

/// Maximum steppers per config
pub const MAX_STEPPERS: usize = 4;

/// Maximum DC motors per config
pub const MAX_DC_MOTORS: usize = 4;

/// Maximum AC motors per config
pub const MAX_AC_MOTORS: usize = 4;

/// Maximum heaters per config
pub const MAX_HEATERS: usize = 4;

/// Motor type for the machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MotorType {
    /// Stepper motors with TMC drivers
    #[default]
    Stepper,
    /// DC motors with PWM control
    Dc,
    /// AC motors with relay control
    Ac,
}

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
    /// Stepper name (e.g., "basket", "z", "x", "lid")
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
    /// Rotation distance in mm per motor rotation
    /// For linear axes (z/lift): leadscrew pitch (e.g., 8mm for T8 leadscrew)
    /// For rotary axes (x/carousel): arc circumference per rotation
    /// For basket: typically 360 (treating degrees as mm for RPM calc)
    pub rotation_distance: u16,
    /// Gear ratio numerator (e.g., 3 for 3:1)
    pub gear_ratio_num: u8,
    /// Gear ratio denominator (e.g., 1 for 3:1)
    pub gear_ratio_den: u8,

    // === Position control (Klipper-style) ===

    /// Minimum valid position in mm (default: 0)
    pub position_min: i32,
    /// Maximum valid position in mm (required for position-controlled steppers)
    pub position_max: Option<i32>,
    /// Location of the endstop in mm (required if endstop_pin is set)
    pub position_endstop: Option<i32>,
    /// Homing speed in mm/s (default: 5)
    pub homing_speed: Option<u16>,
    /// Distance to retract after first endstop contact in mm (default: 5)
    pub homing_retract_dist: Option<u16>,
    /// If true, home in positive direction; if false, home toward zero
    /// Default: auto-detected from position_endstop location
    pub homing_positive_dir: Option<bool>,
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

/// DC motor driver type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DcDriverType {
    /// H-bridge driver (L298N, TB6612, etc.) - supports direction control
    #[default]
    HBridge,
    /// Single MOSFET driver - only supports one direction
    Mosfet,
    /// Dual MOSFET driver - supports direction control
    DualMosfet,
}

/// DC motor hardware configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DcMotorHwConfig {
    /// Motor name (e.g., "basket", "z", "x", "lid")
    pub name: String<MAX_LABEL_LEN>,
    /// PWM output pin for speed control
    pub pwm_pin: PinConfig,
    /// Direction pin (for H-bridge or dual MOSFET)
    pub dir_pin: Option<PinConfig>,
    /// Enable pin (optional, for H-bridge drivers)
    pub enable_pin: Option<PinConfig>,
    /// Driver type
    pub driver_type: DcDriverType,
    /// PWM frequency in Hz (typical: 25000)
    pub pwm_frequency: u32,
    /// Minimum duty cycle percentage (below this the motor won't start)
    pub min_duty: u8,
    /// Soft start ramp time in ms
    pub soft_start_ms: u16,
    /// Soft stop ramp time in ms
    pub soft_stop_ms: u16,
    /// Endstop pin (optional, for position-controlled motors)
    pub endstop_up: Option<PinConfig>,
    /// Second endstop pin (for bi-directional position control)
    pub endstop_down: Option<PinConfig>,
    /// Home endstop (for x-axis/rotational)
    pub endstop_home: Option<PinConfig>,

    // === Position control (Klipper-style) ===

    /// Minimum valid position in mm (default: 0)
    pub position_min: i32,
    /// Maximum valid position in mm (required for position-controlled motors)
    pub position_max: Option<i32>,
    /// Location of the endstop in mm (required if endstop is set)
    pub position_endstop: Option<i32>,
}

/// AC motor relay type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AcRelayType {
    /// Mechanical relay - slower switching, requires debounce
    #[default]
    Mechanical,
    /// Solid State Relay (SSR) - fast switching, no debounce
    Ssr,
}

/// AC motor hardware configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AcMotorHwConfig {
    /// Motor name (e.g., "basket", "z", "x", "lid")
    pub name: String<MAX_LABEL_LEN>,
    /// Relay enable pin (controls motor on/off)
    pub enable_pin: PinConfig,
    /// Direction pin (for reversible motors)
    pub direction_pin: Option<PinConfig>,
    /// Relay type
    pub relay_type: AcRelayType,
    /// Relay is active-high (true) or active-low (false)
    pub active_high: bool,
    /// Endstop pin (optional, for position-controlled motors)
    pub endstop_up: Option<PinConfig>,
    /// Second endstop pin (for bi-directional position control)
    pub endstop_down: Option<PinConfig>,
    /// Home endstop (for x-axis/rotational)
    pub endstop_home: Option<PinConfig>,

    // === Position control (Klipper-style) ===

    /// Minimum valid position in mm (default: 0)
    pub position_min: i32,
    /// Maximum valid position in mm (required for position-controlled motors)
    pub position_max: Option<i32>,
    /// Location of the endstop in mm (required if endstop is set)
    pub position_endstop: Option<i32>,
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
    /// Motor type for this machine
    pub motor_type: MotorType,

    // === Motion Safety ===

    /// Safe Z position for horizontal travel (mm)
    /// The basket lifts to this height before moving between jars.
    /// Should be high enough to clear jar rims and any obstructions.
    /// Typically near stepper.z position_min (top of travel).
    /// If not specified, defaults to stepper.z position_min.
    pub safe_z: Option<i32>,

    // === Hardware ===

    /// Stepper motor configurations (when motor_type = Stepper)
    pub steppers: Vec<StepperHwConfig, MAX_STEPPERS>,
    /// TMC2209 driver configurations (when motor_type = Stepper)
    pub tmc2209s: Vec<Tmc2209HwConfig, MAX_STEPPERS>,
    /// DC motor configurations (when motor_type = Dc)
    pub dc_motors: Vec<DcMotorHwConfig, MAX_DC_MOTORS>,
    /// AC motor configurations (when motor_type = Ac)
    pub ac_motors: Vec<AcMotorHwConfig, MAX_AC_MOTORS>,
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
            motor_type: MotorType::default(),
            safe_z: None,
            steppers: Vec::new(),
            tmc2209s: Vec::new(),
            dc_motors: Vec::new(),
            ac_motors: Vec::new(),
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

    /// Find a DC motor by name
    pub fn find_dc_motor(&self, name: &str) -> Option<&DcMotorHwConfig> {
        self.dc_motors.iter().find(|m| m.name.as_str() == name)
    }

    /// Find an AC motor by name
    pub fn find_ac_motor(&self, name: &str) -> Option<&AcMotorHwConfig> {
        self.ac_motors.iter().find(|m| m.name.as_str() == name)
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

    /// Check if this is an automated machine (has z and x motors)
    pub fn is_automated(&self) -> bool {
        match self.motor_type {
            MotorType::Stepper => {
                self.find_stepper("z").is_some() && self.find_stepper("x").is_some()
            }
            MotorType::Dc => self.find_dc_motor("z").is_some() && self.find_dc_motor("x").is_some(),
            MotorType::Ac => self.find_ac_motor("z").is_some() && self.find_ac_motor("x").is_some(),
        }
    }

    /// Get the basket stepper (for stepper motor machines)
    pub fn basket_stepper(&self) -> Option<&StepperHwConfig> {
        self.find_stepper("basket")
    }

    /// Get the basket DC motor (for DC motor machines)
    pub fn basket_dc_motor(&self) -> Option<&DcMotorHwConfig> {
        self.find_dc_motor("basket")
    }

    /// Get the basket AC motor (for AC motor machines)
    pub fn basket_ac_motor(&self) -> Option<&AcMotorHwConfig> {
        self.find_ac_motor("basket")
    }

    /// Check if a basket motor is configured (any type)
    pub fn has_basket_motor(&self) -> bool {
        match self.motor_type {
            MotorType::Stepper => self.basket_stepper().is_some(),
            MotorType::Dc => self.basket_dc_motor().is_some(),
            MotorType::Ac => self.basket_ac_motor().is_some(),
        }
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
        assert!(config.basket_stepper().is_none());
    }
}
