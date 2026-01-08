//! PID heater controller
//!
//! Implements proportional-integral-derivative control with time-proportioning
//! output for on/off heater control. Uses fixed-point math for Cortex-M0
//! compatibility.

use super::fixed::Fixed32;
use isochron_core::traits::{HeaterController, HeaterOutput, SensorError, TemperatureSensor};

/// PID coefficients
///
/// Stored as Fixed32 for precision in calculations.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PidCoefficients {
    /// Proportional gain (Kp)
    pub kp: Fixed32,
    /// Integral gain (Ki)
    pub ki: Fixed32,
    /// Derivative gain (Kd)
    pub kd: Fixed32,
}

impl PidCoefficients {
    /// Create coefficients from scaled integers (value × 100)
    ///
    /// # Example
    /// ```
    /// use isochron_drivers::heater::pid::PidCoefficients;
    /// // Kp=1.5, Ki=0.1, Kd=0.5
    /// let coeffs = PidCoefficients::from_scaled_100(150, 10, 50);
    /// ```
    pub const fn from_scaled_100(kp_x100: i32, ki_x100: i32, kd_x100: i32) -> Self {
        Self {
            kp: Fixed32::from_scaled_100(kp_x100),
            ki: Fixed32::from_scaled_100(ki_x100),
            kd: Fixed32::from_scaled_100(kd_x100),
        }
    }

    /// Create coefficients from scaled integers (value × 1000)
    ///
    /// Higher precision for small coefficients.
    pub const fn from_scaled_1000(kp_x1000: i32, ki_x1000: i32, kd_x1000: i32) -> Self {
        Self {
            kp: Fixed32::from_scaled_1000(kp_x1000),
            ki: Fixed32::from_scaled_1000(ki_x1000),
            kd: Fixed32::from_scaled_1000(kd_x1000),
        }
    }

    /// Check if any coefficient is non-zero
    pub fn is_configured(&self) -> bool {
        !self.kp.is_zero() || !self.ki.is_zero() || !self.kd.is_zero()
    }
}

/// PID controller configuration
#[derive(Debug, Clone)]
pub struct PidConfig {
    /// PID coefficients
    pub coefficients: PidCoefficients,
    /// Maximum allowed temperature (°C × 10)
    pub max_temp_x10: i16,
    /// PWM period in control loop ticks
    ///
    /// For a 500ms control loop, 20 ticks = 10 second PWM period.
    /// Longer periods reduce relay wear but decrease response.
    pub pwm_period_ticks: u8,
    /// Integral windup limit (°C × 10)
    ///
    /// Prevents integral term from growing too large during setpoint changes
    /// or when heater can't reach target.
    pub integral_limit_x10: i16,
    /// Deadband around setpoint (°C × 10)
    ///
    /// Errors smaller than this are treated as zero to reduce hunting.
    pub deadband_x10: i16,
}

impl Default for PidConfig {
    fn default() -> Self {
        Self {
            coefficients: PidCoefficients::default(),
            max_temp_x10: 550,       // 55.0°C
            pwm_period_ticks: 20,    // 10 seconds at 500ms loop
            integral_limit_x10: 200, // ±20.0°C integral limit
            deadband_x10: 2,         // 0.2°C deadband
        }
    }
}

/// PID controller internal state
#[derive(Debug, Clone, Default)]
struct PidState {
    /// Accumulated integral term
    integral: Fixed32,
    /// Previous error for derivative calculation
    prev_error_x10: i16,
    /// Current PWM duty cycle (0-255)
    duty: u8,
    /// Current position in PWM cycle
    pwm_tick: u8,
}

/// PID heater controller with time-proportioning output
///
/// Since the heater is on/off (not PWM capable), this controller uses
/// time-proportioning: the heater is turned on for a portion of each
/// PWM period proportional to the PID output.
pub struct PidController<S, H> {
    sensor: S,
    heater: H,
    config: PidConfig,
    target_x10: i16,
    enabled: bool,
    state: PidState,
    last_temp_x10: Option<i16>,
    heater_on: bool,
}

impl<S: TemperatureSensor, H: HeaterOutput> PidController<S, H> {
    /// Create a new PID controller
    pub fn new(sensor: S, heater: H, config: PidConfig) -> Self {
        Self {
            sensor,
            heater,
            config,
            target_x10: 0,
            enabled: false,
            state: PidState::default(),
            last_temp_x10: None,
            heater_on: false,
        }
    }

    /// Update PID coefficients
    ///
    /// Resets internal state to prevent integral windup issues.
    pub fn set_coefficients(&mut self, coefficients: PidCoefficients) {
        self.config.coefficients = coefficients;
        self.reset_state();
    }

    /// Get current PID coefficients
    pub fn coefficients(&self) -> &PidCoefficients {
        &self.config.coefficients
    }

    /// Get current duty cycle (0-255)
    ///
    /// Useful for debugging and display.
    pub fn duty(&self) -> u8 {
        self.state.duty
    }

    /// Get access to the underlying sensor
    pub fn sensor(&self) -> &S {
        &self.sensor
    }

    /// Get access to the underlying heater
    pub fn heater(&self) -> &H {
        &self.heater
    }

    /// Reset internal PID state
    fn reset_state(&mut self) {
        self.state = PidState::default();
    }

    /// Calculate PID output
    ///
    /// Returns duty cycle 0-255.
    fn calculate_output(&mut self, temp_x10: i16) -> u8 {
        let error_x10 = self.target_x10 - temp_x10;

        // Apply deadband
        let error_x10 = if error_x10.abs() <= self.config.deadband_x10 {
            0
        } else {
            error_x10
        };

        let error = Fixed32::from_int(error_x10);
        let coeffs = &self.config.coefficients;

        // Proportional term: P = Kp * error
        let p_term = coeffs.kp.mul(error);

        // Integral term: I += Ki * error (with anti-windup)
        let i_contribution = coeffs.ki.mul(error);
        self.state.integral = self.state.integral.saturating_add(i_contribution);

        // Anti-windup: clamp integral
        let integral_limit = Fixed32::from_int(self.config.integral_limit_x10);
        self.state.integral = self.state.integral.clamp(-integral_limit, integral_limit);

        // Derivative term: D = Kd * (error - prev_error)
        // Using derivative on error rather than measurement to avoid
        // derivative kick on setpoint changes.
        let d_error = error_x10 - self.state.prev_error_x10;
        let d_term = coeffs.kd.mul(Fixed32::from_int(d_error));
        self.state.prev_error_x10 = error_x10;

        // Sum all terms
        let output = p_term
            .saturating_add(self.state.integral)
            .saturating_add(d_term);

        // Scale to 0-255 duty cycle
        // Positive output = heating needed
        // Clamp to valid range
        output.to_int().clamp(0, 255) as u8
    }

    /// Apply time-proportioning PWM
    ///
    /// Turns heater on/off based on current position in PWM cycle
    /// and the calculated duty cycle.
    fn apply_pwm(&mut self, duty: u8) {
        self.state.pwm_tick = (self.state.pwm_tick + 1) % self.config.pwm_period_ticks;

        // Calculate threshold for this tick position
        // duty=0 -> always off, duty=255 -> always on
        let threshold = if self.config.pwm_period_ticks > 0 {
            (self.state.pwm_tick as u16 * 255) / self.config.pwm_period_ticks as u16
        } else {
            0
        };

        let should_be_on = (duty as u16) > threshold;

        if should_be_on != self.heater_on {
            self.heater.set_on(should_be_on);
            self.heater_on = should_be_on;
        }
    }
}

impl<S: TemperatureSensor, H: HeaterOutput> HeaterController for PidController<S, H> {
    fn set_target(&mut self, target_c: i16) {
        let new_target_x10 = (target_c * 10).min(self.config.max_temp_x10);

        // Reset integral on significant target change to prevent windup
        if (new_target_x10 - self.target_x10).abs() > 20 {
            // > 2°C change
            self.state.integral = Fixed32::ZERO;
        }

        self.target_x10 = new_target_x10;
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
            self.reset_state();
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_at_target(&self) -> bool {
        if let Some(temp) = self.last_temp_x10 {
            // Within 1°C of target
            (temp - self.target_x10).abs() <= 10
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
            // Don't reset integral - we might be oscillating around max
            return Ok(());
        }

        // Calculate PID output
        let duty = self.calculate_output(temp_x10);
        self.state.duty = duty;

        // Apply time-proportioning PWM
        self.apply_pwm(duty);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSensor {
        temp_x10: i16,
    }

    impl TemperatureSensor for MockSensor {
        fn read_celsius_x10(&mut self) -> Result<i16, SensorError> {
            Ok(self.temp_x10)
        }
    }

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
    fn test_coefficients_from_scaled() {
        let coeffs = PidCoefficients::from_scaled_100(150, 100, 50);
        assert!(coeffs.is_configured());
        assert_eq!(coeffs.kp.to_scaled_100(), 150);
        // Values that are multiples of 100 round-trip perfectly
        assert_eq!(coeffs.ki.to_scaled_100(), 100);
        assert_eq!(coeffs.kd.to_scaled_100(), 50);

        // Test that small values still work (may have slight rounding)
        let small = PidCoefficients::from_scaled_100(10, 10, 10);
        // 10/100 = 0.1, which doesn't round-trip perfectly in Q16.16
        // The error is at most 1 in the scaled-100 representation
        assert!((small.kp.to_scaled_100() - 10).abs() <= 1);
    }

    #[test]
    fn test_pid_heating_needed() {
        let sensor = MockSensor { temp_x10: 400 }; // 40°C
        let heater = MockHeater { on: false };
        let config = PidConfig {
            coefficients: PidCoefficients::from_scaled_100(100, 0, 0), // P-only
            pwm_period_ticks: 1,                                       // Immediate response
            ..Default::default()
        };

        let mut controller = PidController::new(sensor, heater, config);
        controller.set_target(50); // 50°C target
        controller.enable(true);

        // Update should result in heating (temp below target)
        controller.update().unwrap();
        assert!(controller.duty() > 0);
    }

    #[test]
    fn test_pid_at_target() {
        let sensor = MockSensor { temp_x10: 450 }; // 45°C
        let heater = MockHeater { on: false };
        let config = PidConfig {
            coefficients: PidCoefficients::from_scaled_100(100, 0, 0),
            pwm_period_ticks: 1,
            deadband_x10: 50, // 5°C deadband
            ..Default::default()
        };

        let mut controller = PidController::new(sensor, heater, config);
        controller.set_target(45); // 45°C target
        controller.enable(true);

        // Within deadband - should output zero
        controller.update().unwrap();
        assert_eq!(controller.duty(), 0);
        assert!(controller.is_at_target());
    }

    #[test]
    fn test_pid_safety_cutoff() {
        let sensor = MockSensor { temp_x10: 560 }; // 56°C (above 55°C max)
        let heater = MockHeater { on: true };
        let config = PidConfig::default();

        let mut controller = PidController::new(sensor, heater, config);
        controller.set_target(60); // Even with high target
        controller.enable(true);

        controller.update().unwrap();
        assert!(!controller.heater.is_on()); // Should be forced off
    }

    #[test]
    fn test_pid_enable_disable() {
        let sensor = MockSensor { temp_x10: 400 };
        let heater = MockHeater { on: true };
        let config = PidConfig {
            coefficients: PidCoefficients::from_scaled_100(100, 10, 0),
            ..Default::default()
        };

        let mut controller = PidController::new(sensor, heater, config);
        controller.set_target(50);
        controller.enable(true);
        controller.update().unwrap();

        // Disable should turn off heater and reset state
        controller.enable(false);
        assert!(!controller.heater.is_on());
        assert!(!controller.is_enabled());
    }
}
