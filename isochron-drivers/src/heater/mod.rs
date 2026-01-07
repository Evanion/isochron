//! Heater controller implementations

pub mod bang_bang;
pub mod gpio;
// pub mod pid;  // Future

pub use bang_bang::{BangBangConfig, BangBangController};
pub use gpio::{GpioHeater, OutputPin};
