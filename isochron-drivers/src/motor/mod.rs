//! Motor driver implementations
//!
//! This module provides implementations for DC and AC motors:
//!
//! - DC motors: PWM-controlled with soft start/stop
//! - AC motors: Relay-controlled with timing safety

pub mod dc;
pub mod ac;

pub use dc::{DcMotor, DcMotorConfig};
pub use ac::{AcMotor, AcMotorConfig};
