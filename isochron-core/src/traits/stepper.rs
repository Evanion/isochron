//! Stepper motor driver trait
//!
//! This trait abstracts over different stepper driver implementations
//! (TMC2209, TMC2130, A4988, etc.)

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Motor rotation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Direction {
    /// Clockwise rotation
    Clockwise,
    /// Counter-clockwise rotation
    CounterClockwise,
}

impl Direction {
    /// Get the opposite direction
    pub fn opposite(self) -> Self {
        match self {
            Direction::Clockwise => Direction::CounterClockwise,
            Direction::CounterClockwise => Direction::Clockwise,
        }
    }
}

/// Errors that can occur with stepper operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StepperError {
    /// Communication error with driver (UART/SPI)
    CommunicationError,
    /// Motor stall detected
    StallDetected,
    /// Driver overtemperature
    OverTemperature,
    /// Invalid configuration
    InvalidConfig,
}

/// Trait for stepper motor drivers
///
/// Implementations provide motor control capabilities while abstracting
/// the underlying driver interface (UART for TMC2209, SPI for TMC2130, etc.)
pub trait StepperDriver {
    /// Set the target speed in RPM
    ///
    /// The driver should ramp to this speed using the configured acceleration.
    /// A value of 0 stops the motor.
    fn set_rpm(&mut self, rpm: u16);

    /// Get the current target RPM
    fn get_rpm(&self) -> u16;

    /// Set the rotation direction
    ///
    /// Direction should only be changed when the motor is stopped (rpm = 0).
    fn set_direction(&mut self, dir: Direction);

    /// Get the current direction
    fn get_direction(&self) -> Direction;

    /// Enable or disable the motor driver
    ///
    /// When disabled, the motor is free to rotate and does not hold position.
    fn enable(&mut self, enabled: bool);

    /// Check if the motor is enabled
    fn is_enabled(&self) -> bool;

    /// Check if a stall has been detected
    ///
    /// For TMC drivers with StallGuard, this indicates the motor has hit
    /// an obstruction or endstop.
    fn is_stalled(&self) -> bool;

    /// Clear any stall flag
    fn clear_stall(&mut self);

    /// Check if the motor is currently at the target RPM
    fn is_at_speed(&self) -> bool;

    /// Check if the motor is stopped (RPM = 0)
    fn is_stopped(&self) -> bool {
        self.get_rpm() == 0 && self.is_at_speed()
    }
}

/// Extended trait for position-controlled steppers (z, x, lid)
pub trait PositionStepperDriver: StepperDriver {
    /// Move to an absolute position
    ///
    /// Position is in the units defined by the configuration
    /// (mm for linear, degrees for rotational).
    fn move_to(&mut self, position: i32) -> Result<(), StepperError>;

    /// Get the current position
    fn get_position(&self) -> i32;

    /// Start homing sequence
    fn home(&mut self) -> Result<(), StepperError>;

    /// Check if homing is complete
    fn is_homed(&self) -> bool;

    /// Check if a move is in progress
    fn is_moving(&self) -> bool;
}
