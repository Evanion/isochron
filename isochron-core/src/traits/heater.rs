//! Heater and temperature sensor traits

/// Errors that can occur with temperature sensing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SensorError {
    /// Sensor disconnected (open circuit)
    OpenCircuit,
    /// Sensor shorted to ground
    ShortCircuit,
    /// Reading out of expected range
    OutOfRange,
    /// ADC conversion error
    ConversionError,
}

/// Trait for temperature sensors
///
/// Implementations should handle the specific sensor type (NTC thermistor,
/// thermocouple, PT100, etc.)
pub trait TemperatureSensor {
    /// Read the current temperature in degrees Celsius
    ///
    /// Returns a fixed-point value with 0.1°C resolution.
    /// For example, 45.5°C is returned as 455.
    ///
    /// Takes `&mut self` because ADC reads typically require mutable access.
    fn read_celsius_x10(&mut self) -> Result<i16, SensorError>;

    /// Read the current temperature in whole degrees Celsius
    fn read_celsius(&mut self) -> Result<i16, SensorError> {
        self.read_celsius_x10().map(|t| t / 10)
    }

    /// Check if the sensor reading is valid
    fn is_valid(&mut self) -> bool {
        self.read_celsius_x10().is_ok()
    }
}

/// Trait for heater output control
///
/// Implementations control the heater element via GPIO, PWM, or SSR.
pub trait HeaterOutput {
    /// Turn the heater on or off
    fn set_on(&mut self, on: bool);

    /// Check if the heater is currently on
    fn is_on(&self) -> bool;
}

/// Combined heater controller with temperature feedback
///
/// This trait is implemented by controllers that manage both the
/// temperature reading and heater output for closed-loop control.
pub trait HeaterController {
    /// Set the target temperature in degrees Celsius
    fn set_target(&mut self, target_c: i16);

    /// Get the current target temperature
    fn get_target(&self) -> i16;

    /// Get the current actual temperature
    ///
    /// Takes `&mut self` because reading the sensor requires mutable access.
    fn get_current(&mut self) -> Result<i16, SensorError>;

    /// Enable or disable the heater controller
    ///
    /// When disabled, the heater output is forced off.
    fn enable(&mut self, enabled: bool);

    /// Check if the controller is enabled
    fn is_enabled(&self) -> bool;

    /// Check if the heater is currently at target temperature (within hysteresis)
    fn is_at_target(&self) -> bool;

    /// Update the control loop
    ///
    /// This should be called periodically (e.g., every 100ms).
    fn update(&mut self) -> Result<(), SensorError>;
}
