//! Profile scheduler
//!
//! Converts user-visible profiles into execution segments and manages
//! program execution.

pub mod executor;
pub mod segment;

pub use executor::{
    ExecutionPhase, HeaterCommand, MotorCommand, Scheduler, StepState, MAX_SEGMENTS,
};
pub use segment::{DirectionMode, Segment, SpinOffConfig, generate_segments};
