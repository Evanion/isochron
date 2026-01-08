//! Configuration type definitions
//!
//! These types represent the machine configuration. Configuration is stored
//! in flash as postcard-serialized binary data.

use heapless::String;

use crate::scheduler::{DirectionMode, SpinOffConfig};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Maximum label length
pub const MAX_LABEL_LEN: usize = 16;

/// Maximum profiles per config
pub const MAX_PROFILES: usize = 8;

/// Maximum programs per config
pub const MAX_PROGRAMS: usize = 8;

/// Maximum steps per program
pub const MAX_STEPS_PER_PROGRAM: usize = 8;

/// Maximum jars
pub const MAX_JARS: usize = 8;

/// Profile type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ProfileType {
    #[default]
    Clean,
    Rinse,
    Dry,
}

/// Profile configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProfileConfig {
    /// Display label
    pub label: String<MAX_LABEL_LEN>,
    /// Profile type
    pub profile_type: ProfileType,
    /// Target RPM
    pub rpm: u16,
    /// Duration in seconds
    pub time_s: u16,
    /// Direction mode
    pub direction: DirectionMode,
    /// Number of iterations (for Alternate mode)
    pub iterations: u8,
    /// Target temperature for drying (°C)
    pub temperature_c: Option<i16>,
    /// Optional spin-off configuration
    pub spinoff: Option<SpinOffConfig>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            label: String::new(),
            profile_type: ProfileType::Clean,
            rpm: 120,
            time_s: 180,
            direction: DirectionMode::Alternate,
            iterations: 3,
            temperature_c: None,
            spinoff: None,
        }
    }
}

/// Jar position configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JarConfig {
    /// Jar name/identifier
    pub name: String<MAX_LABEL_LEN>,
    /// Tower/position motor position (degrees from home)
    pub tower_pos: i32,
    /// Lift motor position (mm down from top)
    pub lift_pos: i32,
    /// Associated heater name (optional)
    pub heater: Option<String<MAX_LABEL_LEN>>,
    /// Associated ultrasonic module name (optional)
    pub ultrasonic: Option<String<MAX_LABEL_LEN>>,
}

/// Program step (jar + profile pair)
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProgramStep {
    /// Jar name
    pub jar: String<MAX_LABEL_LEN>,
    /// Profile name
    pub profile: String<MAX_LABEL_LEN>,
}

/// Program configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProgramConfig {
    /// Display label
    pub label: String<MAX_LABEL_LEN>,
    /// Steps in this program
    pub steps: heapless::Vec<ProgramStep, MAX_STEPS_PER_PROGRAM>,
}

impl Default for ProgramConfig {
    fn default() -> Self {
        Self {
            label: String::new(),
            steps: heapless::Vec::new(),
        }
    }
}

/// Heater control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum HeaterControlMode {
    /// Simple on/off control with hysteresis
    #[default]
    BangBang,
    /// PID control with time-proportioning output
    Pid,
}

/// Heater configuration
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HeaterConfig {
    /// Heater name/identifier
    pub name: String<MAX_LABEL_LEN>,
    /// Control mode
    pub control: HeaterControlMode,
    /// Maximum allowed temperature (°C)
    pub max_temp: i16,
    /// Hysteresis for bang-bang control (°C)
    pub hysteresis: i16,
    /// PID proportional gain (value × 100, e.g., 150 = 1.50)
    ///
    /// Only used when control mode is Pid. If None, uses calibration
    /// values from flash or defaults.
    pub pid_kp_x100: Option<i16>,
    /// PID integral gain (value × 100, e.g., 10 = 0.10)
    pub pid_ki_x100: Option<i16>,
    /// PID derivative gain (value × 100, e.g., 50 = 0.50)
    pub pid_kd_x100: Option<i16>,
}

/// UI configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UiConfig {
    /// RPM adjustment step
    pub rpm_step: u16,
    /// Time adjustment step (seconds)
    pub time_step_s: u16,
    /// Temperature adjustment step (°C)
    pub temp_step_c: i16,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            rpm_step: 10,
            time_step_s: 30,
            temp_step_c: 5,
        }
    }
}

/// Machine capabilities (determined from config)
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MachineCapabilities {
    /// Has lift motor
    pub has_lift: bool,
    /// Has tower/position motor
    pub has_tower: bool,
    /// Has lid motor
    pub has_lid: bool,
    /// Number of heaters
    pub heater_count: u8,
    /// Is an automated machine (has lift and tower)
    pub is_automated: bool,
}

impl MachineCapabilities {
    /// Determine capabilities from available hardware
    pub fn from_config(has_lift: bool, has_tower: bool, has_lid: bool, heater_count: u8) -> Self {
        Self {
            has_lift,
            has_tower,
            has_lid,
            heater_count,
            is_automated: has_lift && has_tower,
        }
    }
}
