//! Motor driver traits
//!
//! This module defines traits for different motor types:
//! - DC motors (PWM-controlled)
//! - AC motors (relay-controlled)
//!
//! For stepper motors, see the [`stepper`] module.

use super::Direction;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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

/// Errors that can occur with motor operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MotorError {
    /// Motor is disabled
    Disabled,
    /// Safety interlock prevented operation
    Interlock,
    /// Endstop triggered unexpectedly
    EndstopTriggered,
    /// Motor not homed
    NotHomed,
    /// Invalid speed value
    InvalidSpeed,
    /// Relay switch too fast
    SwitchTooFast,
}

/// Base trait for all motor drivers (DC and AC)
///
/// This provides common functionality shared by DC and AC motors.
/// Note that stepper motors use the separate [`StepperDriver`] trait.
pub trait MotorDriver {
    /// Set the rotation direction
    ///
    /// For motors without direction control (single MOSFET, some AC motors),
    /// this may be a no-op.
    fn set_direction(&mut self, dir: Direction);

    /// Get the current direction
    fn get_direction(&self) -> Direction;

    /// Enable or disable the motor driver
    ///
    /// When disabled, the motor should coast to a stop.
    fn enable(&mut self, enabled: bool);

    /// Check if the motor is enabled
    fn is_enabled(&self) -> bool;

    /// Start the motor (begin running)
    fn start(&mut self) -> Result<(), MotorError>;

    /// Stop the motor (begin stopping)
    fn stop(&mut self);

    /// Check if the motor is currently running
    fn is_running(&self) -> bool;

    /// Check if the motor is fully stopped
    fn is_stopped(&self) -> bool {
        !self.is_running()
    }
}

/// Trait for DC motors with PWM speed control
///
/// DC motors use a duty cycle (0-100%) for speed control, with optional
/// soft start/stop ramping for smooth acceleration/deceleration.
pub trait DcMotorDriver: MotorDriver {
    /// Set the target speed as a percentage (0-100)
    ///
    /// The actual duty cycle may be scaled by the configured minimum
    /// duty cycle (the threshold where the motor actually starts moving).
    fn set_speed(&mut self, percent: u8);

    /// Get the current target speed percentage
    fn get_speed(&self) -> u8;

    /// Get the current actual speed (may differ during ramping)
    fn get_actual_speed(&self) -> u8;

    /// Check if the motor has reached its target speed
    fn is_at_speed(&self) -> bool {
        self.get_speed() == self.get_actual_speed()
    }

    /// Update the motor state (call periodically for soft start/stop)
    ///
    /// Returns the current duty cycle to apply to PWM.
    fn update(&mut self) -> u8;
}

/// Trait for AC motors with relay control
///
/// AC motors have no speed control - they run at fixed speed.
/// Control is limited to on/off and optionally direction.
pub trait AcMotorDriver: MotorDriver {
    /// Check if direction control is available
    fn has_direction_control(&self) -> bool;

    /// Get the minimum time between relay switches (for debounce)
    fn min_switch_delay_ms(&self) -> u32;

    /// Check if enough time has passed since last relay operation
    fn can_switch(&self) -> bool;

    /// Update the motor state (call periodically for timing/safety)
    fn update(&mut self);
}

/// State for a DC motor with soft start/stop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DcMotorState {
    /// Motor is stopped
    #[default]
    Stopped,
    /// Motor is ramping up to target speed
    Starting,
    /// Motor is running at target speed
    Running,
    /// Motor is ramping down to stop
    Stopping,
}

/// State for an AC motor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AcMotorState {
    /// Motor is off
    #[default]
    Off,
    /// Motor is running
    On,
    /// Motor is in switch delay (waiting before next operation)
    SwitchDelay,
}
