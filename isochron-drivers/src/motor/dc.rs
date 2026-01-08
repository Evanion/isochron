//! DC motor driver with PWM speed control
//!
//! This driver provides:
//! - PWM duty cycle control (0-100%)
//! - Soft start/stop ramping for smooth acceleration
//! - Direction control for H-bridge drivers
//! - Minimum duty cycle handling (below which motor won't start)
//!
//! # Usage
//!
//! The driver is updated by calling `update()` periodically (typically every
//! millisecond). This returns the current duty cycle to apply to the PWM output.
//!
//! ```ignore
//! let mut motor = DcMotor::new(config);
//! motor.enable(true);
//! motor.set_speed(80); // 80% speed
//! motor.start()?;
//!
//! // In periodic timer interrupt:
//! let duty = motor.update();
//! pwm.set_duty(duty);
//! ```

use isochron_core::traits::{DcMotorDriver, DcMotorState, Direction, MotorDriver, MotorError};

/// DC motor driver configuration
#[derive(Debug, Clone)]
pub struct DcMotorConfig {
    /// Minimum duty cycle percentage (below this the motor won't start)
    pub min_duty: u8,
    /// Soft start ramp time in ms (0 = instant)
    pub soft_start_ms: u16,
    /// Soft stop ramp time in ms (0 = instant)
    pub soft_stop_ms: u16,
    /// Whether direction control is available
    pub has_direction: bool,
}

impl Default for DcMotorConfig {
    fn default() -> Self {
        Self {
            min_duty: 20,
            soft_start_ms: 500,
            soft_stop_ms: 300,
            has_direction: true,
        }
    }
}

/// DC motor driver state
///
/// This struct manages the motor state and provides PWM duty cycle
/// calculation with soft start/stop ramping.
pub struct DcMotor {
    config: DcMotorConfig,
    /// Target speed (0-100%)
    target_speed: u8,
    /// Current actual speed (0-100%, during ramping)
    actual_speed: u8,
    /// Current direction
    direction: Direction,
    /// Whether the driver is enabled
    enabled: bool,
    /// Current motor state
    state: DcMotorState,
    /// Accumulated time for ramping (in ms)
    ramp_time_ms: u32,
    /// Speed when ramping started
    ramp_start_speed: u8,
    /// Speed target for ramping
    ramp_end_speed: u8,
}

impl DcMotor {
    /// Create a new DC motor driver
    pub fn new(config: DcMotorConfig) -> Self {
        Self {
            config,
            target_speed: 0,
            actual_speed: 0,
            direction: Direction::Clockwise,
            enabled: false,
            state: DcMotorState::Stopped,
            ramp_time_ms: 0,
            ramp_start_speed: 0,
            ramp_end_speed: 0,
        }
    }

    /// Get the current motor state
    pub fn state(&self) -> DcMotorState {
        self.state
    }

    /// Get the configuration
    pub fn config(&self) -> &DcMotorConfig {
        &self.config
    }

    /// Check if direction control is available
    pub fn has_direction_control(&self) -> bool {
        self.config.has_direction
    }

    /// Get the direction pin state (true = forward/CW)
    pub fn direction_pin_state(&self) -> bool {
        self.direction == Direction::Clockwise
    }

    /// Get the enable pin state (true = enabled)
    pub fn enable_pin_state(&self) -> bool {
        self.enabled && self.actual_speed > 0
    }

    /// Scale the speed percentage to actual duty cycle
    ///
    /// Maps 0-100% to min_duty-100%, so that 0% = off and 100% = full power,
    /// with the dead zone below min_duty handled.
    fn scale_duty(&self, speed: u8) -> u8 {
        if speed == 0 {
            0
        } else {
            // Scale speed to range [min_duty, 100]
            let min = self.config.min_duty as u32;
            let range = 100 - min;
            let scaled = min + (speed as u32 * range / 100);
            scaled.min(100) as u8
        }
    }

    /// Calculate the ramped speed for the current time
    fn calculate_ramp_speed(&self, ramp_time_total_ms: u16) -> u8 {
        if ramp_time_total_ms == 0 {
            // No ramping, instant change
            return self.ramp_end_speed;
        }

        let progress = (self.ramp_time_ms * 100) / ramp_time_total_ms as u32;
        let progress = progress.min(100) as u8;

        if self.ramp_start_speed < self.ramp_end_speed {
            // Ramping up
            let delta = self.ramp_end_speed - self.ramp_start_speed;
            self.ramp_start_speed + (delta as u32 * progress as u32 / 100) as u8
        } else {
            // Ramping down
            let delta = self.ramp_start_speed - self.ramp_end_speed;
            self.ramp_start_speed - (delta as u32 * progress as u32 / 100) as u8
        }
    }

    /// Start the ramp to a new target speed
    fn start_ramp(&mut self, target: u8) {
        self.ramp_start_speed = self.actual_speed;
        self.ramp_end_speed = target;
        self.ramp_time_ms = 0;
    }

    /// Update for a specific time delta (in ms)
    ///
    /// Returns the current duty cycle to apply.
    pub fn update_with_delta(&mut self, delta_ms: u32) -> u8 {
        if !self.enabled {
            self.actual_speed = 0;
            self.state = DcMotorState::Stopped;
            return 0;
        }

        match self.state {
            DcMotorState::Stopped => {
                // Nothing to do
            }
            DcMotorState::Starting => {
                self.ramp_time_ms += delta_ms;
                self.actual_speed = self.calculate_ramp_speed(self.config.soft_start_ms);

                if self.actual_speed >= self.target_speed {
                    self.actual_speed = self.target_speed;
                    self.state = DcMotorState::Running;
                }
            }
            DcMotorState::Running => {
                // Check if target changed
                if self.target_speed == 0 {
                    // Start stopping
                    self.start_ramp(0);
                    self.state = DcMotorState::Stopping;
                } else if self.actual_speed != self.target_speed {
                    // Speed changed while running - start new ramp
                    self.start_ramp(self.target_speed);
                    if self.target_speed > self.actual_speed {
                        self.state = DcMotorState::Starting;
                    } else {
                        self.state = DcMotorState::Stopping;
                    }
                }
            }
            DcMotorState::Stopping => {
                self.ramp_time_ms += delta_ms;
                self.actual_speed = self.calculate_ramp_speed(self.config.soft_stop_ms);

                if self.actual_speed == 0 {
                    self.state = DcMotorState::Stopped;
                } else if self.ramp_time_ms >= self.config.soft_stop_ms as u32 {
                    self.actual_speed = 0;
                    self.state = DcMotorState::Stopped;
                }
            }
        }

        self.scale_duty(self.actual_speed)
    }
}

impl MotorDriver for DcMotor {
    fn set_direction(&mut self, dir: Direction) {
        // Only allow direction change when stopped
        if self.state == DcMotorState::Stopped {
            self.direction = dir;
        }
    }

    fn get_direction(&self) -> Direction {
        self.direction
    }

    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.target_speed = 0;
            self.actual_speed = 0;
            self.state = DcMotorState::Stopped;
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn start(&mut self) -> Result<(), MotorError> {
        if !self.enabled {
            return Err(MotorError::Disabled);
        }

        if self.target_speed == 0 {
            return Err(MotorError::InvalidSpeed);
        }

        if self.state == DcMotorState::Stopped {
            self.start_ramp(self.target_speed);
            self.state = DcMotorState::Starting;
        }

        Ok(())
    }

    fn stop(&mut self) {
        if self.state != DcMotorState::Stopped {
            self.target_speed = 0;
            self.start_ramp(0);
            self.state = DcMotorState::Stopping;
        }
    }

    fn is_running(&self) -> bool {
        matches!(
            self.state,
            DcMotorState::Starting | DcMotorState::Running | DcMotorState::Stopping
        )
    }

    fn is_stopped(&self) -> bool {
        self.state == DcMotorState::Stopped
    }
}

impl DcMotorDriver for DcMotor {
    fn set_speed(&mut self, percent: u8) {
        self.target_speed = percent.min(100);

        // If running, the update() will handle the speed change
        // If stopped, just set the target for when we start
    }

    fn get_speed(&self) -> u8 {
        self.target_speed
    }

    fn get_actual_speed(&self) -> u8 {
        self.actual_speed
    }

    fn is_at_speed(&self) -> bool {
        self.actual_speed == self.target_speed
    }

    fn update(&mut self) -> u8 {
        // Default 1ms update interval
        self.update_with_delta(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let motor = DcMotor::new(DcMotorConfig::default());

        assert!(!motor.is_enabled());
        assert!(motor.is_stopped());
        assert_eq!(motor.get_speed(), 0);
        assert_eq!(motor.get_actual_speed(), 0);
        assert_eq!(motor.state(), DcMotorState::Stopped);
    }

    #[test]
    fn test_enable_disable() {
        let mut motor = DcMotor::new(DcMotorConfig::default());

        motor.enable(true);
        assert!(motor.is_enabled());

        motor.enable(false);
        assert!(!motor.is_enabled());
        assert!(motor.is_stopped());
    }

    #[test]
    fn test_start_requires_enable() {
        let mut motor = DcMotor::new(DcMotorConfig::default());

        motor.set_speed(50);
        let result = motor.start();
        assert_eq!(result, Err(MotorError::Disabled));
    }

    #[test]
    fn test_start_requires_speed() {
        let mut motor = DcMotor::new(DcMotorConfig::default());

        motor.enable(true);
        let result = motor.start();
        assert_eq!(result, Err(MotorError::InvalidSpeed));
    }

    #[test]
    fn test_soft_start() {
        let config = DcMotorConfig {
            min_duty: 0,
            soft_start_ms: 100,
            soft_stop_ms: 100,
            has_direction: true,
        };
        let mut motor = DcMotor::new(config);

        motor.enable(true);
        motor.set_speed(100);
        motor.start().unwrap();

        assert_eq!(motor.state(), DcMotorState::Starting);

        // After 50ms, should be at ~50%
        for _ in 0..50 {
            motor.update();
        }
        assert!(motor.get_actual_speed() >= 40 && motor.get_actual_speed() <= 60);
        assert_eq!(motor.state(), DcMotorState::Starting);

        // After 100ms total, should be at 100%
        for _ in 0..50 {
            motor.update();
        }
        assert_eq!(motor.get_actual_speed(), 100);
        assert_eq!(motor.state(), DcMotorState::Running);
    }

    #[test]
    fn test_soft_stop() {
        let config = DcMotorConfig {
            min_duty: 0,
            soft_start_ms: 0, // Instant start
            soft_stop_ms: 100,
            has_direction: true,
        };
        let mut motor = DcMotor::new(config);

        motor.enable(true);
        motor.set_speed(100);
        motor.start().unwrap();
        motor.update(); // Get to running state

        assert_eq!(motor.state(), DcMotorState::Running);
        assert_eq!(motor.get_actual_speed(), 100);

        motor.stop();
        assert_eq!(motor.state(), DcMotorState::Stopping);

        // After 50ms, should be at ~50%
        for _ in 0..50 {
            motor.update();
        }
        assert!(motor.get_actual_speed() <= 60);

        // After 100ms total, should be at 0%
        for _ in 0..60 {
            motor.update();
        }
        assert_eq!(motor.get_actual_speed(), 0);
        assert_eq!(motor.state(), DcMotorState::Stopped);
    }

    #[test]
    fn test_instant_start_stop() {
        let config = DcMotorConfig {
            min_duty: 0,
            soft_start_ms: 0,
            soft_stop_ms: 0,
            has_direction: true,
        };
        let mut motor = DcMotor::new(config);

        motor.enable(true);
        motor.set_speed(80);
        motor.start().unwrap();
        motor.update();

        // Should immediately be at speed
        assert_eq!(motor.get_actual_speed(), 80);
        assert_eq!(motor.state(), DcMotorState::Running);

        motor.stop();
        motor.update();

        // Should immediately stop
        assert_eq!(motor.get_actual_speed(), 0);
        assert_eq!(motor.state(), DcMotorState::Stopped);
    }

    #[test]
    fn test_duty_scaling() {
        let config = DcMotorConfig {
            min_duty: 20,
            soft_start_ms: 0,
            soft_stop_ms: 0,
            has_direction: true,
        };
        let mut motor = DcMotor::new(config);

        motor.enable(true);

        // 0% should be 0 (off) - can't start with 0 speed
        motor.set_speed(0);
        assert_eq!(motor.start(), Err(MotorError::InvalidSpeed));
        let duty = motor.update();
        assert_eq!(duty, 0);

        // 100% should be 100
        motor.set_speed(100);
        motor.start().unwrap();
        let duty = motor.update();
        assert_eq!(duty, 100);

        // Stop and restart at 50%
        motor.stop();
        motor.update();

        motor.set_speed(50);
        motor.start().unwrap();
        let duty = motor.update();
        // 50% should be scaled: 20 + (50% of 80) = 20 + 40 = 60
        assert_eq!(duty, 60);
    }

    #[test]
    fn test_direction_change_only_when_stopped() {
        let mut motor = DcMotor::new(DcMotorConfig::default());

        motor.set_direction(Direction::CounterClockwise);
        assert_eq!(motor.get_direction(), Direction::CounterClockwise);

        motor.enable(true);
        motor.set_speed(50);
        motor.start().unwrap();
        motor.update();

        // Try to change direction while running - should be ignored
        motor.set_direction(Direction::Clockwise);
        assert_eq!(motor.get_direction(), Direction::CounterClockwise);

        // Stop and change
        motor.stop();
        while motor.state() != DcMotorState::Stopped {
            motor.update();
        }

        motor.set_direction(Direction::Clockwise);
        assert_eq!(motor.get_direction(), Direction::Clockwise);
    }
}
