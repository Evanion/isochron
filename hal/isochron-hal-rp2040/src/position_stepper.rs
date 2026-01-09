//! Position-tracking stepper motor driver
//!
//! Wraps a PioStepper to add position tracking and homing capability.
//! Used for Z-axis (lift/lower) and X-axis (jar positioning) control.

use embassy_rp::gpio::{Input, Pin, Pull};
use embassy_rp::pio::{Common, Instance, PioPin, StateMachine};
use embassy_rp::Peri;

use crate::pio::StepGeneratorConfig;
use crate::stepper::PioStepper;

/// Position stepper configuration
#[derive(Debug, Clone)]
pub struct PositionStepperConfig {
    /// Base stepper config (step/dir/enable pins, steps_per_rev)
    pub stepper: StepGeneratorConfig,
    /// Steps per millimeter of travel
    pub steps_per_mm: u32,
    /// Minimum position in mm (can be negative)
    pub position_min_mm: i32,
    /// Maximum position in mm
    pub position_max_mm: i32,
    /// Position at endstop after homing
    pub position_endstop_mm: i32,
    /// Homing speed in mm/s
    pub homing_speed_mm_s: u16,
    /// Distance to retract after hitting endstop
    pub homing_retract_mm: u16,
    /// Normal move speed in mm/s
    pub move_speed_mm_s: u16,
    /// Endstop is active low (normally high, goes low when triggered)
    pub endstop_active_low: bool,
    /// Home toward max position (true) or min position (false)
    pub home_to_max: bool,
}

impl Default for PositionStepperConfig {
    fn default() -> Self {
        Self {
            stepper: StepGeneratorConfig::default(),
            steps_per_mm: 80, // Common for GT2 belt + 20T pulley
            position_min_mm: 0,
            position_max_mm: 200,
            position_endstop_mm: 0,
            homing_speed_mm_s: 10,
            homing_retract_mm: 5,
            move_speed_mm_s: 50,
            endstop_active_low: true,
            home_to_max: false,
        }
    }
}

/// Position stepper state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PositionState {
    /// Not homed, position unknown
    NotHomed,
    /// Homing in progress - moving toward endstop
    HomingSeek,
    /// Homing in progress - retracting from endstop
    HomingRetract,
    /// Homed and idle at known position
    Idle,
    /// Moving to target position
    Moving,
}

/// Position stepper errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PositionError {
    /// Target position is out of configured bounds
    OutOfBounds,
    /// Motor has not been homed
    NotHomed,
    /// Endstop not triggered during homing
    HomingFailed,
    /// Move timed out
    Timeout,
}

/// Position-tracking stepper driver
///
/// Wraps a PioStepper and adds:
/// - Position tracking (estimated from time and speed)
/// - Homing using an endstop switch
/// - Absolute position moves
pub struct PositionStepper<'d, PIO: Instance, const SM: usize> {
    /// Underlying stepper driver
    stepper: PioStepper<'d, PIO, SM>,
    /// Endstop input pin
    endstop: Input<'d>,
    /// Configuration
    config: PositionStepperConfig,
    /// Current position in micrometers (for precision)
    position_um: i32,
    /// Target position in micrometers
    target_um: i32,
    /// Current state
    state: PositionState,
    /// Move start timestamp (for timeout detection)
    move_start_ms: u64,
    /// Direction: true = positive, false = negative
    direction_positive: bool,
}

impl<'d, PIO: Instance, const SM: usize> PositionStepper<'d, PIO, SM> {
    /// Create a new position-tracking stepper
    ///
    /// # Arguments
    /// * `common` - PIO common resources
    /// * `sm` - State machine to use
    /// * `step_pin` - GPIO pin for step pulses
    /// * `dir_pin` - GPIO pin for direction
    /// * `enable_pin` - GPIO pin for enable
    /// * `endstop_pin` - GPIO pin for endstop switch
    /// * `config` - Position stepper configuration
    pub fn new<STEP: PioPin, DIR: Pin, EN: Pin, END: Pin>(
        common: &mut Common<'d, PIO>,
        sm: StateMachine<'d, PIO, SM>,
        step_pin: Peri<'d, STEP>,
        dir_pin: Peri<'d, DIR>,
        enable_pin: Peri<'d, EN>,
        endstop_pin: Peri<'d, END>,
        config: PositionStepperConfig,
    ) -> Self {
        let stepper = PioStepper::new(common, sm, step_pin, dir_pin, enable_pin, config.stepper.clone());

        // Setup endstop with pull-up (most endstops are normally open, active low)
        let endstop = Input::new(endstop_pin, Pull::Up);

        Self {
            stepper,
            endstop,
            config,
            position_um: 0,
            target_um: 0,
            state: PositionState::NotHomed,
            move_start_ms: 0,
            direction_positive: true,
        }
    }

    /// Get current state
    pub fn state(&self) -> PositionState {
        self.state
    }

    /// Check if homed
    pub fn is_homed(&self) -> bool {
        !matches!(
            self.state,
            PositionState::NotHomed | PositionState::HomingSeek | PositionState::HomingRetract
        )
    }

    /// Check if moving
    pub fn is_moving(&self) -> bool {
        matches!(
            self.state,
            PositionState::Moving | PositionState::HomingSeek | PositionState::HomingRetract
        )
    }

    /// Check if idle (homed and not moving)
    pub fn is_idle(&self) -> bool {
        self.state == PositionState::Idle
    }

    /// Get current position in mm
    pub fn position_mm(&self) -> i32 {
        self.position_um / 1000
    }

    /// Get current position in micrometers
    pub fn position_um(&self) -> i32 {
        self.position_um
    }

    /// Check if endstop is triggered
    pub fn endstop_triggered(&self) -> bool {
        let level = self.endstop.is_high();
        if self.config.endstop_active_low {
            !level // Active low: triggered when low
        } else {
            level // Active high: triggered when high
        }
    }

    /// Check if position is in bounds
    pub fn is_in_bounds(&self, position_mm: i32) -> bool {
        position_mm >= self.config.position_min_mm && position_mm <= self.config.position_max_mm
    }

    /// Enable the motor
    pub fn enable(&mut self) {
        self.stepper.enable();
    }

    /// Disable the motor
    pub fn disable(&mut self) {
        self.stepper.disable();
    }

    /// Start homing sequence
    ///
    /// Returns Ok(()) if homing started, Err if already homing or moving.
    pub fn start_homing(&mut self) -> Result<(), PositionError> {
        if self.is_moving() {
            return Err(PositionError::NotHomed); // Can't home while moving
        }

        self.enable();
        self.state = PositionState::HomingSeek;

        // Set direction toward endstop
        let direction = self.config.home_to_max;
        self.direction_positive = direction;
        self.stepper.set_direction(!direction); // Stepper direction is inverted from position direction

        // Start moving at homing speed
        let speed_mm_s = self.config.homing_speed_mm_s as u32;
        let freq_hz = speed_mm_s * self.config.steps_per_mm;
        self.stepper.set_frequency(freq_hz);

        Ok(())
    }

    /// Start move to absolute position
    ///
    /// Returns Ok(()) if move started, Err if not homed or out of bounds.
    pub fn move_to(&mut self, position_mm: i32) -> Result<(), PositionError> {
        if !self.is_homed() {
            return Err(PositionError::NotHomed);
        }

        if !self.is_in_bounds(position_mm) {
            return Err(PositionError::OutOfBounds);
        }

        let target_um = position_mm * 1000;
        if target_um == self.position_um {
            return Ok(()); // Already at target
        }

        self.target_um = target_um;
        self.state = PositionState::Moving;

        // Determine direction
        let direction = target_um > self.position_um;
        self.direction_positive = direction;
        self.stepper.set_direction(!direction); // Stepper direction inverted

        // Start moving at configured speed
        let speed_mm_s = self.config.move_speed_mm_s as u32;
        let freq_hz = speed_mm_s * self.config.steps_per_mm;
        self.stepper.set_frequency(freq_hz);

        Ok(())
    }

    /// Stop motor immediately
    pub fn stop(&mut self) {
        self.stepper.stop();
        if self.is_moving() && self.is_homed() {
            self.state = PositionState::Idle;
        }
    }

    /// Emergency stop - disables motor
    pub fn emergency_stop(&mut self) {
        self.stepper.stop();
        self.stepper.disable();
        // Don't change state - position may be unknown
    }

    /// Update position tracking and state machine
    ///
    /// Call this periodically (e.g., every 10ms) to update position estimation
    /// and handle state transitions.
    ///
    /// # Arguments
    /// * `delta_ms` - Time elapsed since last update in milliseconds
    /// * `current_ms` - Current timestamp in milliseconds (for timeout detection)
    ///
    /// # Returns
    /// * `Ok(true)` - Operation completed (homing done, move done)
    /// * `Ok(false)` - Operation in progress
    /// * `Err` - Error occurred
    pub fn update(&mut self, delta_ms: u32, current_ms: u64) -> Result<bool, PositionError> {
        match self.state {
            PositionState::NotHomed | PositionState::Idle => Ok(false),

            PositionState::HomingSeek => {
                if self.endstop_triggered() {
                    // Hit endstop, start retract
                    self.stepper.stop();

                    // Reverse direction for retract
                    self.direction_positive = !self.direction_positive;
                    self.stepper.set_direction(self.direction_positive);

                    // Move retract distance at homing speed
                    let speed_mm_s = self.config.homing_speed_mm_s as u32;
                    let freq_hz = speed_mm_s * self.config.steps_per_mm;
                    self.stepper.set_frequency(freq_hz);

                    // Calculate retract target
                    let retract_um = (self.config.homing_retract_mm as i32) * 1000;
                    self.target_um = if self.config.home_to_max {
                        self.config.position_endstop_mm * 1000 - retract_um
                    } else {
                        self.config.position_endstop_mm * 1000 + retract_um
                    };

                    self.position_um = self.config.position_endstop_mm * 1000;
                    self.move_start_ms = current_ms;
                    self.state = PositionState::HomingRetract;
                }
                Ok(false)
            }

            PositionState::HomingRetract => {
                // Update position during retract
                self.update_position(delta_ms);

                // Check if retract complete
                let distance_um = (self.target_um - self.position_um).abs();
                if distance_um < 100 {
                    // Within 0.1mm
                    self.stepper.stop();
                    self.position_um = self.target_um;
                    self.state = PositionState::Idle;
                    return Ok(true); // Homing complete
                }

                // Timeout check (5 seconds should be plenty for retract)
                if current_ms - self.move_start_ms > 5000 {
                    self.stepper.stop();
                    return Err(PositionError::HomingFailed);
                }

                Ok(false)
            }

            PositionState::Moving => {
                // Update position during move
                self.update_position(delta_ms);

                // Check if move complete
                let distance_um = (self.target_um - self.position_um).abs();
                if distance_um < 100 {
                    // Within 0.1mm
                    self.stepper.stop();
                    self.position_um = self.target_um;
                    self.state = PositionState::Idle;
                    return Ok(true); // Move complete
                }

                // Safety: check for endstop during move (shouldn't happen)
                if self.endstop_triggered() {
                    self.stepper.stop();
                    // Update position to endstop position
                    self.position_um = self.config.position_endstop_mm * 1000;
                    self.state = PositionState::Idle;
                    return Ok(true); // Stopped at endstop
                }

                Ok(false)
            }
        }
    }

    /// Update position estimate based on elapsed time and speed
    fn update_position(&mut self, delta_ms: u32) {
        if !self.stepper.is_running() {
            return;
        }

        // Calculate distance traveled
        // speed = freq_hz / steps_per_mm (in mm/s)
        // distance = speed * time (in mm)
        // We work in micrometers for precision

        let freq_hz = self.stepper.current_freq();
        if freq_hz == 0 || self.config.steps_per_mm == 0 {
            return;
        }

        // distance_um = (freq_hz * delta_ms * 1000) / (steps_per_mm * 1000)
        //             = (freq_hz * delta_ms) / steps_per_mm
        let distance_um = (freq_hz as i64 * delta_ms as i64 * 1000) / (self.config.steps_per_mm as i64 * 1000);

        if self.direction_positive {
            self.position_um += distance_um as i32;
        } else {
            self.position_um -= distance_um as i32;
        }

        // Clamp to bounds
        let min_um = self.config.position_min_mm * 1000;
        let max_um = self.config.position_max_mm * 1000;
        self.position_um = self.position_um.clamp(min_um, max_um);
    }

    /// Get configuration reference
    pub fn config(&self) -> &PositionStepperConfig {
        &self.config
    }

    /// Get underlying stepper reference (for advanced control)
    pub fn stepper(&self) -> &PioStepper<'d, PIO, SM> {
        &self.stepper
    }

    /// Get mutable stepper reference (for advanced control)
    pub fn stepper_mut(&mut self) -> &mut PioStepper<'d, PIO, SM> {
        &mut self.stepper
    }
}
