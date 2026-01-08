//! AC motor driver with relay control
//!
//! This driver provides:
//! - On/off control via relay (mechanical or SSR)
//! - Minimum switch delay to protect relays from rapid switching
//! - Optional direction control for reversible motors
//! - Safety interlock support
//!
//! # Usage
//!
//! The driver is updated by calling `update()` periodically (typically every
//! millisecond). This manages timing and returns the relay state.
//!
//! ```ignore
//! let mut motor = AcMotor::new(config);
//! motor.enable(true);
//! motor.start()?;
//!
//! // In periodic timer interrupt:
//! motor.update();
//! relay_pin.set(motor.relay_state());
//! ```
//!
//! # Safety
//!
//! AC motors controlled by relays require careful timing:
//! - Mechanical relays need debounce time (~100ms minimum between switches)
//! - SSRs can switch faster but still benefit from delay (~10ms)
//! - Direction changes must wait for motor to stop spinning (inertia)

use isochron_core::traits::{AcMotorDriver, AcMotorState, Direction, MotorDriver, MotorError};

/// AC motor driver configuration
#[derive(Debug, Clone)]
pub struct AcMotorConfig {
    /// Relay type affects minimum switch delay
    pub relay_type: AcRelayType,
    /// Minimum delay between relay switches (ms)
    pub min_switch_delay_ms: u32,
    /// Relay is active-high (true) or active-low (false)
    pub active_high: bool,
    /// Whether direction control is available
    pub has_direction: bool,
}

/// AC motor relay type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcRelayType {
    /// Mechanical relay - slower switching, longer delay needed
    #[default]
    Mechanical,
    /// Solid State Relay (SSR) - faster switching
    Ssr,
}

impl Default for AcMotorConfig {
    fn default() -> Self {
        Self {
            relay_type: AcRelayType::Mechanical,
            min_switch_delay_ms: 100,
            active_high: true,
            has_direction: false,
        }
    }
}

impl AcMotorConfig {
    /// Create config for a mechanical relay
    pub fn mechanical() -> Self {
        Self {
            relay_type: AcRelayType::Mechanical,
            min_switch_delay_ms: 100,
            active_high: true,
            has_direction: false,
        }
    }

    /// Create config for an SSR
    pub fn ssr() -> Self {
        Self {
            relay_type: AcRelayType::Ssr,
            min_switch_delay_ms: 10,
            active_high: true,
            has_direction: false,
        }
    }
}

/// AC motor driver state
///
/// This struct manages the motor state and provides safe relay control
/// with timing protection.
pub struct AcMotor {
    config: AcMotorConfig,
    /// Current direction
    direction: Direction,
    /// Desired direction (for after motor stops)
    desired_direction: Direction,
    /// Whether the driver is enabled
    enabled: bool,
    /// Current motor state
    state: AcMotorState,
    /// Time since last relay switch (ms)
    time_since_switch_ms: u32,
    /// Time remaining in switch delay (ms)
    switch_delay_remaining_ms: u32,
    /// Whether relay is currently active
    relay_active: bool,
}

impl AcMotor {
    /// Create a new AC motor driver
    pub fn new(config: AcMotorConfig) -> Self {
        Self {
            config,
            direction: Direction::Clockwise,
            desired_direction: Direction::Clockwise,
            enabled: false,
            state: AcMotorState::Off,
            time_since_switch_ms: u32::MAX, // Allow immediate first switch
            switch_delay_remaining_ms: 0,
            relay_active: false,
        }
    }

    /// Get the current motor state
    pub fn state(&self) -> AcMotorState {
        self.state
    }

    /// Get the configuration
    pub fn config(&self) -> &AcMotorConfig {
        &self.config
    }

    /// Get the relay output state (accounting for active-high/low)
    pub fn relay_state(&self) -> bool {
        if self.config.active_high {
            self.relay_active
        } else {
            !self.relay_active
        }
    }

    /// Get the direction pin state (true = forward/CW)
    pub fn direction_pin_state(&self) -> bool {
        self.direction == Direction::Clockwise
    }

    /// Switch the relay state
    fn switch_relay(&mut self, active: bool) {
        if self.relay_active != active {
            self.relay_active = active;
            self.time_since_switch_ms = 0;
            self.switch_delay_remaining_ms = self.config.min_switch_delay_ms;
        }
    }

    /// Update for a specific time delta (in ms)
    pub fn update_with_delta(&mut self, delta_ms: u32) {
        // Update timing
        self.time_since_switch_ms = self.time_since_switch_ms.saturating_add(delta_ms);

        if self.switch_delay_remaining_ms > 0 {
            self.switch_delay_remaining_ms = self.switch_delay_remaining_ms.saturating_sub(delta_ms);
        }

        if !self.enabled {
            self.switch_relay(false);
            self.state = AcMotorState::Off;
            return;
        }

        match self.state {
            AcMotorState::Off => {
                // Check if direction needs to change
                if self.direction != self.desired_direction {
                    self.direction = self.desired_direction;
                }
            }
            AcMotorState::On => {
                // Motor is running
            }
            AcMotorState::SwitchDelay => {
                if self.switch_delay_remaining_ms == 0 {
                    // Delay complete, transition to target state
                    if self.relay_active {
                        self.state = AcMotorState::On;
                    } else {
                        self.state = AcMotorState::Off;
                    }
                }
            }
        }
    }
}

impl MotorDriver for AcMotor {
    fn set_direction(&mut self, dir: Direction) {
        // Store desired direction - actual change happens when stopped
        self.desired_direction = dir;

        // If already stopped, apply immediately
        if self.state == AcMotorState::Off {
            self.direction = dir;
        }
    }

    fn get_direction(&self) -> Direction {
        self.direction
    }

    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.switch_relay(false);
            self.state = AcMotorState::Off;
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn start(&mut self) -> Result<(), MotorError> {
        if !self.enabled {
            return Err(MotorError::Disabled);
        }

        if !self.can_switch() {
            return Err(MotorError::SwitchTooFast);
        }

        if self.state == AcMotorState::Off {
            self.switch_relay(true);
            self.state = AcMotorState::SwitchDelay;
        }

        Ok(())
    }

    fn stop(&mut self) {
        if self.state != AcMotorState::Off {
            self.switch_relay(false);
            self.state = AcMotorState::SwitchDelay;
        }
    }

    fn is_running(&self) -> bool {
        self.state == AcMotorState::On
    }

    fn is_stopped(&self) -> bool {
        self.state == AcMotorState::Off
    }
}

impl AcMotorDriver for AcMotor {
    fn has_direction_control(&self) -> bool {
        self.config.has_direction
    }

    fn min_switch_delay_ms(&self) -> u32 {
        self.config.min_switch_delay_ms
    }

    fn can_switch(&self) -> bool {
        self.time_since_switch_ms >= self.config.min_switch_delay_ms
    }

    fn update(&mut self) {
        // Default 1ms update interval
        self.update_with_delta(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let motor = AcMotor::new(AcMotorConfig::default());

        assert!(!motor.is_enabled());
        assert!(motor.is_stopped());
        assert_eq!(motor.state(), AcMotorState::Off);
        assert!(!motor.relay_state());
    }

    #[test]
    fn test_enable_disable() {
        let mut motor = AcMotor::new(AcMotorConfig::default());

        motor.enable(true);
        assert!(motor.is_enabled());

        motor.enable(false);
        assert!(!motor.is_enabled());
        assert!(motor.is_stopped());
    }

    #[test]
    fn test_start_requires_enable() {
        let mut motor = AcMotor::new(AcMotorConfig::default());

        let result = motor.start();
        assert_eq!(result, Err(MotorError::Disabled));
    }

    #[test]
    fn test_start_and_run() {
        let config = AcMotorConfig {
            min_switch_delay_ms: 10,
            ..Default::default()
        };
        let mut motor = AcMotor::new(config);

        motor.enable(true);
        motor.start().unwrap();

        assert_eq!(motor.state(), AcMotorState::SwitchDelay);
        assert!(motor.relay_state()); // Relay should be active

        // After switch delay, should be running
        for _ in 0..15 {
            motor.update();
        }
        assert_eq!(motor.state(), AcMotorState::On);
        assert!(motor.is_running());
    }

    #[test]
    fn test_stop() {
        let config = AcMotorConfig {
            min_switch_delay_ms: 10,
            ..Default::default()
        };
        let mut motor = AcMotor::new(config);

        motor.enable(true);
        motor.start().unwrap();

        // Wait for switch delay
        for _ in 0..15 {
            motor.update();
        }
        assert!(motor.is_running());

        motor.stop();
        assert_eq!(motor.state(), AcMotorState::SwitchDelay);
        assert!(!motor.relay_state()); // Relay should be off

        // After switch delay, should be stopped
        for _ in 0..15 {
            motor.update();
        }
        assert!(motor.is_stopped());
    }

    #[test]
    fn test_switch_too_fast_protection() {
        let config = AcMotorConfig {
            min_switch_delay_ms: 100,
            ..Default::default()
        };
        let mut motor = AcMotor::new(config);

        motor.enable(true);
        motor.start().unwrap();

        // Wait for switch delay
        for _ in 0..110 {
            motor.update();
        }

        motor.stop();

        // Try to start again immediately - should fail
        for _ in 0..50 {
            motor.update();
        }

        let result = motor.start();
        assert_eq!(result, Err(MotorError::SwitchTooFast));

        // Wait for full delay
        for _ in 0..100 {
            motor.update();
        }

        let result = motor.start();
        assert!(result.is_ok());
    }

    #[test]
    fn test_direction_change_only_when_stopped() {
        let config = AcMotorConfig {
            min_switch_delay_ms: 10,
            has_direction: true,
            ..Default::default()
        };
        let mut motor = AcMotor::new(config);

        motor.enable(true);
        motor.set_direction(Direction::CounterClockwise);
        assert_eq!(motor.get_direction(), Direction::CounterClockwise);

        motor.start().unwrap();

        // Wait for running
        for _ in 0..15 {
            motor.update();
        }

        // Try to change direction while running - should queue but not apply
        motor.set_direction(Direction::Clockwise);
        assert_eq!(motor.get_direction(), Direction::CounterClockwise);

        // Stop and wait
        motor.stop();
        for _ in 0..15 {
            motor.update();
        }

        // Now direction should change
        motor.update();
        assert_eq!(motor.get_direction(), Direction::Clockwise);
    }

    #[test]
    fn test_active_low_relay() {
        let config = AcMotorConfig {
            active_high: false,
            min_switch_delay_ms: 10,
            ..Default::default()
        };
        let mut motor = AcMotor::new(config);

        // When off, active-low relay should output HIGH
        assert!(motor.relay_state());

        motor.enable(true);
        motor.start().unwrap();

        // When on, active-low relay should output LOW
        assert!(!motor.relay_state());
    }

    #[test]
    fn test_ssr_config() {
        let config = AcMotorConfig::ssr();

        assert_eq!(config.relay_type, AcRelayType::Ssr);
        assert_eq!(config.min_switch_delay_ms, 10);
        assert!(config.active_high);
    }

    #[test]
    fn test_mechanical_config() {
        let config = AcMotorConfig::mechanical();

        assert_eq!(config.relay_type, AcRelayType::Mechanical);
        assert_eq!(config.min_switch_delay_ms, 100);
    }
}
