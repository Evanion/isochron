//! Board-agnostic core logic for the watch cleaner firmware
//!
//! This crate contains all application logic that does not depend on
//! specific hardware implementations:
//!
//! - Hardware abstraction traits (stepper, heater, sensor)
//! - State machine for program execution
//! - Profile scheduler
//! - Motion planning (acceleration math)
//! - Safety monitoring logic
//! - Configuration type definitions

#![no_std]
#![deny(unsafe_code)]

pub mod config;
pub mod motion;
pub mod safety;
pub mod scheduler;
pub mod state;
pub mod traits;
