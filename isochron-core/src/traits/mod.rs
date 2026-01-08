//! Hardware abstraction traits
//!
//! These traits define the interface between the application logic
//! and hardware-specific implementations.

pub mod display;
pub mod heater;
pub mod motor;
pub mod stepper;

pub use display::DisplayDriver;
pub use heater::{HeaterController, HeaterOutput, SensorError, TemperatureSensor};
pub use motor::{
    AcMotorDriver, AcMotorState, AcRelayType, DcDriverType, DcMotorDriver, DcMotorState,
    MotorDriver, MotorError,
};
pub use stepper::{Direction, PositionStepperDriver, StepperDriver, StepperError};
