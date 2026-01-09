//! Motion planning and position control
//!
//! Acceleration and deceleration profiles for smooth motor control,
//! plus position control types for automated axis movement.

pub mod planner;
pub mod position;

pub use planner::{MotionPlanner, MotionState};
pub use position::{
    Axis, HomingCommand, PositionCommand, PositionConfig, PositionError, PositionStatus,
};
