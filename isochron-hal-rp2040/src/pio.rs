//! PIO-based step pulse generator
//!
//! Uses RP2040's Programmable I/O to generate precise step pulses
//! for stepper motors. This offloads timing-critical pulse generation
//! from the CPU.
//!
//! # Architecture
//!
//! Each stepper motor gets its own PIO state machine (SM), but all SMs
//! share the same PIO program loaded once. The RP2040 has 2 PIO blocks
//! with 4 SMs each, so we can drive up to 8 steppers (though we only need 4).
//!
//! The step frequency is controlled by the PIO clock divider. The pulse
//! width is fixed at a minimum of 2.5µs (safe for all stepper drivers).

// Embassy PIO imports for when we implement the actual driver
// use embassy_rp::pio::{
//     Common, Config, Direction as PioDirection, Instance, PioPin, ShiftConfig,
//     ShiftDirection, StateMachine,
// };

/// System clock frequency (RP2040 default)
pub const SYS_CLK_HZ: u32 = 125_000_000;

/// Minimum pulse width in nanoseconds (2.5µs is safe for all drivers)
pub const MIN_PULSE_WIDTH_NS: u32 = 2500;

/// Maximum step frequency in Hz (limited by pulse width)
pub const MAX_STEP_FREQ_HZ: u32 = 200_000;

/// PIO step generator configuration
#[derive(Debug, Clone)]
pub struct StepGeneratorConfig {
    /// Step GPIO pin
    pub step_pin: u8,
    /// Direction GPIO pin
    pub dir_pin: u8,
    /// Enable GPIO pin
    pub enable_pin: u8,
    /// Enable pin is active low
    pub enable_inverted: bool,
    /// Steps per revolution (including microstepping)
    pub steps_per_rev: u32,
}

impl Default for StepGeneratorConfig {
    fn default() -> Self {
        Self {
            step_pin: 11,
            dir_pin: 10,
            enable_pin: 12,
            enable_inverted: true,
            steps_per_rev: 200 * 16, // 200 full steps * 16 microsteps
        }
    }
}

/// Calculate the clock divider for a target frequency
///
/// The PIO program runs at SYS_CLK / divider Hz.
/// With 2 instructions per cycle (set high, set low), the step frequency is:
/// freq = SYS_CLK / (divider * 2)
///
/// Therefore: divider = SYS_CLK / (freq * 2)
///
/// Returns (integer_part, fractional_part) for the 16.8 fixed-point divider.
pub fn calc_clock_divider(freq_hz: u32) -> (u16, u8) {
    if freq_hz == 0 {
        return (0xFFFF, 0xFF); // Maximum divider = stopped
    }

    // divider = SYS_CLK / (freq * instructions_per_step)
    // We use 2 instructions per step (high + low)
    // To get 8-bit fractional precision, multiply by 256 first
    // divider * 256 = (SYS_CLK * 256) / (freq * 2)
    let divisor = freq_hz * 2;
    let divider_x256 = (SYS_CLK_HZ as u64 * 256) / (divisor as u64);

    // Split into integer and fractional parts
    let int_part = (divider_x256 / 256) as u32;
    let frac_part = (divider_x256 % 256) as u32;

    // Clamp to valid range
    let int_part = int_part.min(0xFFFF) as u16;
    let frac_part = frac_part.min(0xFF) as u8;

    (int_part, frac_part)
}

/// Convert RPM to step frequency in Hz
pub fn rpm_to_freq(rpm: u16, steps_per_rev: u32) -> u32 {
    // freq = rpm * steps_per_rev / 60
    (rpm as u32) * steps_per_rev / 60
}

/// Convert step frequency to RPM
pub fn freq_to_rpm(freq_hz: u32, steps_per_rev: u32) -> u16 {
    if steps_per_rev == 0 {
        return 0;
    }
    // rpm = freq * 60 / steps_per_rev
    ((freq_hz * 60) / steps_per_rev) as u16
}

/// PIO program for step pulse generation
///
/// This is a minimal 2-instruction program that generates a square wave.
/// The step frequency is controlled by the clock divider.
#[rustfmt::skip]
pub const STEP_PROGRAM: &[u16] = &[
    // .wrap_target
    0xE001, // set pins, 1      ; Set step pin high
    0xE000, // set pins, 0      ; Set step pin low
    // .wrap
];

/// Assemble the step generator PIO program
///
/// Returns the program ready to be loaded into a PIO block.
pub fn step_program() -> &'static [u16] {
    STEP_PROGRAM
}

/// High-level step generator wrapper
///
/// This provides a simple interface for controlling a stepper motor
/// without directly managing the PIO state machine.
pub struct StepGenerator {
    config: StepGeneratorConfig,
    /// Current frequency in Hz (steps per second)
    current_freq_hz: u32,
    /// Target frequency in Hz
    target_freq_hz: u32,
    /// Is the generator running
    running: bool,
    /// Current direction (true = CW, false = CCW)
    direction_cw: bool,
    /// Is enabled
    enabled: bool,
}

impl StepGenerator {
    /// Create a new step generator
    pub fn new(config: StepGeneratorConfig) -> Self {
        Self {
            config,
            current_freq_hz: 0,
            target_freq_hz: 0,
            running: false,
            direction_cw: true,
            enabled: false,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &StepGeneratorConfig {
        &self.config
    }

    /// Get current frequency in Hz
    pub fn current_freq(&self) -> u32 {
        self.current_freq_hz
    }

    /// Get target frequency in Hz
    pub fn target_freq(&self) -> u32 {
        self.target_freq_hz
    }

    /// Set the target frequency in Hz
    ///
    /// Note: This only updates the internal target. The actual PIO
    /// state machine must be updated separately by the driver.
    pub fn set_frequency(&mut self, freq_hz: u32) {
        self.target_freq_hz = freq_hz.min(MAX_STEP_FREQ_HZ);
    }

    /// Update current frequency to match target
    pub fn sync_frequency(&mut self) {
        self.current_freq_hz = self.target_freq_hz;
    }

    /// Convert RPM to step frequency
    pub fn rpm_to_freq(&self, rpm: u16) -> u32 {
        rpm_to_freq(rpm, self.config.steps_per_rev)
    }

    /// Convert frequency to RPM
    pub fn freq_to_rpm(&self, freq_hz: u32) -> u16 {
        freq_to_rpm(freq_hz, self.config.steps_per_rev)
    }

    /// Get current RPM
    pub fn current_rpm(&self) -> u16 {
        self.freq_to_rpm(self.current_freq_hz)
    }

    /// Set speed in RPM
    pub fn set_rpm(&mut self, rpm: u16) {
        let freq = self.rpm_to_freq(rpm);
        self.set_frequency(freq);
    }

    /// Set direction
    pub fn set_direction(&mut self, clockwise: bool) {
        self.direction_cw = clockwise;
    }

    /// Get direction
    pub fn direction(&self) -> bool {
        self.direction_cw
    }

    /// Set enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Mark as running
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
        if !running {
            self.current_freq_hz = 0;
        }
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Check if at target frequency
    pub fn is_at_target(&self) -> bool {
        self.current_freq_hz == self.target_freq_hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_divider() {
        // At 1kHz step freq, we need clock divider of 62500
        // (125MHz / (1000 * 2) = 62500)
        let (int_part, _frac_part) = calc_clock_divider(1000);
        assert_eq!(int_part, 62500);

        // At 100kHz step freq, we need clock divider of 625
        let (int_part, _) = calc_clock_divider(100_000);
        assert_eq!(int_part, 625);
    }

    #[test]
    fn test_rpm_conversion() {
        let steps_per_rev = 200 * 16; // 3200 steps/rev

        // At 60 RPM, we need 3200 steps/second
        let freq = rpm_to_freq(60, steps_per_rev);
        assert_eq!(freq, 3200);

        // At 120 RPM, we need 6400 steps/second
        let freq = rpm_to_freq(120, steps_per_rev);
        assert_eq!(freq, 6400);

        // Reverse: 3200 Hz = 60 RPM
        let rpm = freq_to_rpm(3200, steps_per_rev);
        assert_eq!(rpm, 60);
    }

    #[test]
    fn test_step_generator() {
        let config = StepGeneratorConfig::default();
        let mut gen = StepGenerator::new(config);

        assert!(!gen.is_running());
        assert_eq!(gen.current_freq(), 0);

        gen.set_rpm(120);
        assert!(gen.target_freq() > 0);

        gen.sync_frequency();
        assert_eq!(gen.current_freq(), gen.target_freq());
    }
}
