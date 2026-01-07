//! Hardware driver implementations
//!
//! This crate provides concrete implementations of the traits defined
//! in cleaner-core for various hardware components:
//!
//! - Stepper drivers (TMC2209, TMC2130, A4988)
//! - Heater controllers (bang-bang, PID)
//! - Temperature sensors (NTC thermistor)
//! - Accessories (ultrasonic, neopixel, fan, speaker)

#![no_std]
#![deny(unsafe_code)]

pub mod stepper;
pub mod heater;
pub mod sensor;
pub mod accessory;
