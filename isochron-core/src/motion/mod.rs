//! Motion planning
//!
//! Acceleration and deceleration profiles for smooth motor control.

pub mod planner;

pub use planner::{MotionPlanner, MotionState};
