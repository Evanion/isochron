//! Position control types for automated axis movement
//!
//! These types define the command/status interface between the controller
//! and position motor tasks for Z and X axis control.

/// Axis identifier for position commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Axis {
    /// Z axis (vertical lift/lower)
    Z,
    /// X axis (horizontal jar positioning)
    X,
}

/// Command to move an axis to a position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PositionCommand {
    /// Target axis
    pub axis: Axis,
    /// Target position in millimeters (can be negative for below-zero positions)
    pub target_mm: i32,
}

impl PositionCommand {
    /// Create a Z axis position command
    pub fn z(target_mm: i32) -> Self {
        Self {
            axis: Axis::Z,
            target_mm,
        }
    }

    /// Create an X axis position command
    pub fn x(target_mm: i32) -> Self {
        Self {
            axis: Axis::X,
            target_mm,
        }
    }
}

/// Command to home an axis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HomingCommand {
    /// Home Z axis (lift to top)
    HomeZ,
    /// Home X axis (move to reference position)
    HomeX,
}

/// Status of a position operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PositionStatus {
    /// Move completed successfully
    Complete(Axis),
    /// Homing completed successfully
    Homed(Axis),
    /// Position operation failed
    Error {
        axis: Axis,
        kind: PositionError,
    },
}

impl PositionStatus {
    /// Create a move complete status
    pub fn complete(axis: Axis) -> Self {
        Self::Complete(axis)
    }

    /// Create a homing complete status
    pub fn homed(axis: Axis) -> Self {
        Self::Homed(axis)
    }

    /// Create an error status
    pub fn error(axis: Axis, kind: PositionError) -> Self {
        Self::Error { axis, kind }
    }

    /// Check if this is a successful status
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Complete(_) | Self::Homed(_))
    }

    /// Check if this is an error status
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Get the axis for this status
    pub fn axis(&self) -> Axis {
        match self {
            Self::Complete(axis) | Self::Homed(axis) | Self::Error { axis, .. } => *axis,
        }
    }
}

/// Position operation error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PositionError {
    /// Endstop not triggered during homing
    EndstopNotTriggered,
    /// Motor stall detected (sensorless homing)
    StallDetected,
    /// Target position outside configured bounds
    OutOfBounds,
    /// Operation timed out
    Timeout,
    /// Motor not homed (position unknown)
    NotHomed,
}

/// Configuration for position control
#[derive(Debug, Clone, Copy)]
pub struct PositionConfig {
    /// Steps per millimeter of travel
    pub steps_per_mm: u32,
    /// Minimum allowed position in mm
    pub position_min_mm: i32,
    /// Maximum allowed position in mm
    pub position_max_mm: i32,
    /// Position value at endstop (after homing)
    pub position_endstop_mm: i32,
    /// Homing speed in mm/s
    pub homing_speed_mm_s: u16,
    /// Distance to retract after hitting endstop during homing
    pub homing_retract_mm: u16,
    /// Normal move speed in mm/s
    pub move_speed_mm_s: u16,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            steps_per_mm: 80, // Common for GT2 belt + 20T pulley + 1/16 microstep
            position_min_mm: 0,
            position_max_mm: 200,
            position_endstop_mm: 0,
            homing_speed_mm_s: 10,
            homing_retract_mm: 5,
            move_speed_mm_s: 50,
        }
    }
}

impl PositionConfig {
    /// Check if a position is within bounds
    pub fn is_in_bounds(&self, position_mm: i32) -> bool {
        position_mm >= self.position_min_mm && position_mm <= self.position_max_mm
    }

    /// Clamp a position to valid bounds
    pub fn clamp(&self, position_mm: i32) -> i32 {
        position_mm.clamp(self.position_min_mm, self.position_max_mm)
    }

    /// Calculate steps for a given distance in mm
    pub fn mm_to_steps(&self, mm: i32) -> i32 {
        mm * self.steps_per_mm as i32
    }

    /// Calculate mm from steps
    pub fn steps_to_mm(&self, steps: i32) -> i32 {
        steps / self.steps_per_mm as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_command_constructors() {
        let z_cmd = PositionCommand::z(100);
        assert_eq!(z_cmd.axis, Axis::Z);
        assert_eq!(z_cmd.target_mm, 100);

        let x_cmd = PositionCommand::x(50);
        assert_eq!(x_cmd.axis, Axis::X);
        assert_eq!(x_cmd.target_mm, 50);
    }

    #[test]
    fn test_position_status() {
        let complete = PositionStatus::complete(Axis::Z);
        assert!(complete.is_success());
        assert!(!complete.is_error());
        assert_eq!(complete.axis(), Axis::Z);

        let error = PositionStatus::error(Axis::X, PositionError::Timeout);
        assert!(!error.is_success());
        assert!(error.is_error());
        assert_eq!(error.axis(), Axis::X);
    }

    #[test]
    fn test_position_config_bounds() {
        let config = PositionConfig {
            position_min_mm: -10,
            position_max_mm: 100,
            ..Default::default()
        };

        assert!(config.is_in_bounds(50));
        assert!(config.is_in_bounds(-10));
        assert!(config.is_in_bounds(100));
        assert!(!config.is_in_bounds(-11));
        assert!(!config.is_in_bounds(101));
    }

    #[test]
    fn test_position_config_clamp() {
        let config = PositionConfig {
            position_min_mm: 0,
            position_max_mm: 100,
            ..Default::default()
        };

        assert_eq!(config.clamp(50), 50);
        assert_eq!(config.clamp(-10), 0);
        assert_eq!(config.clamp(150), 100);
    }

    #[test]
    fn test_mm_steps_conversion() {
        let config = PositionConfig {
            steps_per_mm: 80,
            ..Default::default()
        };

        assert_eq!(config.mm_to_steps(10), 800);
        assert_eq!(config.steps_to_mm(800), 10);
    }
}
