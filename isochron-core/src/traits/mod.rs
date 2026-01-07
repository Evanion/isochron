//! Hardware abstraction traits
//!
//! These traits define the interface between the application logic
//! and hardware-specific implementations.

pub mod stepper;
pub mod heater;
pub mod display;

pub use stepper::{Direction, StepperDriver, StepperError};
pub use heater::{HeaterController, HeaterOutput, SensorError, TemperatureSensor};
pub use display::DisplayDriver;
