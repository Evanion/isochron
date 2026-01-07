//! Bang-bang heater controller
//!
//! Simple on/off control with hysteresis for temperature regulation.

use isochron_core::traits::{HeaterController, HeaterOutput, SensorError, TemperatureSensor};

/// Bang-bang controller configuration
#[derive(Debug, Clone)]
pub struct BangBangConfig {
    /// Maximum allowed temperature (°C × 10)
    pub max_temp_x10: i16,
    /// Hysteresis (°C × 10)
    pub hysteresis_x10: i16,
}

impl Default for BangBangConfig {
    fn default() -> Self {
        Self {
            max_temp_x10: 550, // 55.0°C
            hysteresis_x10: 20, // 2.0°C
        }
    }
}

/// Bang-bang heater controller
///
/// Turns heater on when temperature drops below (target - hysteresis),
/// and off when temperature rises above (target + hysteresis).
pub struct BangBangController<S, H> {
    sensor: S,
    heater: H,
    config: BangBangConfig,
    target_x10: i16,
    enabled: bool,
    heater_on: bool,
    last_temp_x10: Option<i16>,
}

impl<S: TemperatureSensor, H: HeaterOutput> BangBangController<S, H> {
    /// Create a new bang-bang controller
    pub fn new(sensor: S, heater: H, config: BangBangConfig) -> Self {
        Self {
            sensor,
            heater,
            config,
            target_x10: 0,
            enabled: false,
            heater_on: false,
            last_temp_x10: None,
        }
    }

    /// Get access to the underlying sensor
    pub fn sensor(&self) -> &S {
        &self.sensor
    }

    /// Get access to the underlying heater
    pub fn heater(&self) -> &H {
        &self.heater
    }
}

impl<S: TemperatureSensor, H: HeaterOutput> HeaterController for BangBangController<S, H> {
    fn set_target(&mut self, target_c: i16) {
        // Convert to x10 format
        self.target_x10 = target_c * 10;

        // Clamp to max
        if self.target_x10 > self.config.max_temp_x10 {
            self.target_x10 = self.config.max_temp_x10;
        }
    }

    fn get_target(&self) -> i16 {
        self.target_x10 / 10
    }

    fn get_current(&mut self) -> Result<i16, SensorError> {
        self.sensor.read_celsius()
    }

    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.heater.set_on(false);
            self.heater_on = false;
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_at_target(&self) -> bool {
        if let Some(temp) = self.last_temp_x10 {
            let diff = (temp - self.target_x10).abs();
            diff <= self.config.hysteresis_x10
        } else {
            false
        }
    }

    fn update(&mut self) -> Result<(), SensorError> {
        // Read current temperature
        let temp_x10 = self.sensor.read_celsius_x10()?;
        self.last_temp_x10 = Some(temp_x10);

        if !self.enabled {
            return Ok(());
        }

        // Safety check: never exceed max temperature
        if temp_x10 >= self.config.max_temp_x10 {
            self.heater.set_on(false);
            self.heater_on = false;
            return Ok(());
        }

        // Bang-bang control with hysteresis
        let low_threshold = self.target_x10 - self.config.hysteresis_x10;
        let high_threshold = self.target_x10 + self.config.hysteresis_x10;

        if temp_x10 < low_threshold {
            // Below target - hysteresis: turn on
            self.heater.set_on(true);
            self.heater_on = true;
        } else if temp_x10 > high_threshold {
            // Above target + hysteresis: turn off
            self.heater.set_on(false);
            self.heater_on = false;
        }
        // Otherwise, maintain current state (hysteresis band)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock sensor for testing
    struct MockSensor {
        temp_x10: i16,
        valid: bool,
    }

    impl TemperatureSensor for MockSensor {
        fn read_celsius_x10(&mut self) -> Result<i16, SensorError> {
            if self.valid {
                Ok(self.temp_x10)
            } else {
                Err(SensorError::OpenCircuit)
            }
        }
    }

    // Mock heater for testing
    struct MockHeater {
        on: bool,
    }

    impl HeaterOutput for MockHeater {
        fn set_on(&mut self, on: bool) {
            self.on = on;
        }

        fn is_on(&self) -> bool {
            self.on
        }
    }

    #[test]
    fn test_controller_enable_disable() {
        let sensor = MockSensor {
            temp_x10: 400,
            valid: true,
        };
        let heater = MockHeater { on: false };
        let mut controller = BangBangController::new(sensor, heater, BangBangConfig::default());

        controller.set_target(45);
        controller.enable(true);
        controller.update().unwrap();

        // Temperature (40°C) is below target (45°C) - hysteresis
        assert!(controller.heater.is_on());

        // Disable controller
        controller.enable(false);
        assert!(!controller.heater.is_on());
    }
}
