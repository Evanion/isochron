//! PIO-based stepper motor driver
//!
//! Uses RP2040's Programmable I/O to generate precise step pulses.
//! Each stepper gets its own state machine for independent control.

use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::pio::{Common, Config, Direction as PioDirection, Instance, PioPin, StateMachine};
use embassy_rp::Peri;
use fixed::types::U24F8;

use crate::pio::{calc_clock_divider, StepGeneratorConfig, MAX_STEP_FREQ_HZ};

/// PIO stepper driver
///
/// Controls a stepper motor using PIO for precise step pulse timing.
/// Direction and enable are handled via GPIO.
pub struct PioStepper<'d, PIO: Instance, const SM: usize> {
    /// PIO state machine for step generation
    sm: StateMachine<'d, PIO, SM>,
    /// Direction GPIO output
    dir_pin: Output<'d>,
    /// Enable GPIO output
    enable_pin: Output<'d>,
    /// Configuration
    config: StepGeneratorConfig,
    /// Current frequency in Hz
    current_freq_hz: u32,
    /// Is currently running
    running: bool,
    /// Current direction (true = CW)
    direction_cw: bool,
}

impl<'d, PIO: Instance, const SM: usize> PioStepper<'d, PIO, SM> {
    /// Create a new PIO stepper driver
    ///
    /// # Arguments
    /// * `common` - PIO common resources (for loading program)
    /// * `sm` - State machine to use
    /// * `step_pin` - GPIO pin for step pulses (must be PIO-capable)
    /// * `dir_pin` - GPIO pin for direction control
    /// * `enable_pin` - GPIO pin for enable control
    /// * `config` - Stepper configuration
    pub fn new<STEP: PioPin, DIR: Pin, EN: Pin>(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        step_pin: Peri<'d, STEP>,
        dir_pin: Peri<'d, DIR>,
        enable_pin: Peri<'d, EN>,
        config: StepGeneratorConfig,
    ) -> Self {
        // Load the step pulse program using pio macro
        // This 2-instruction program generates a square wave on the step pin
        let prg = pio::pio_asm!(
            ".wrap_target",
            "set pins, 1", // Set step pin high
            "set pins, 0", // Set step pin low
            ".wrap"
        );

        let installed = common.load_program(&prg.program);

        // Create the PIO pin for the step output
        let step_pio_pin = common.make_pio_pin(step_pin);

        // Configure state machine
        let mut cfg = Config::default();
        cfg.use_program(&installed, &[&step_pio_pin]);
        cfg.set_set_pins(&[&step_pio_pin]);

        // Start with maximum divider (effectively stopped)
        // FixedU32<U8> has 24 integer bits and 8 fractional bits
        cfg.clock_divider = U24F8::from_bits(0xFFFF_FF00);

        sm.set_config(&cfg);
        sm.set_pin_dirs(PioDirection::Out, &[&step_pio_pin]);

        // Setup direction pin - start CW
        let dir_pin = Output::new(dir_pin, Level::Low);

        // Setup enable pin - start disabled
        let enable_level = if config.enable_inverted {
            Level::High // Active low, so high = disabled
        } else {
            Level::Low // Active high, so low = disabled
        };
        let enable_pin = Output::new(enable_pin, enable_level);

        Self {
            sm,
            dir_pin,
            enable_pin,
            config,
            current_freq_hz: 0,
            running: false,
            direction_cw: true,
        }
    }

    /// Enable the stepper driver
    pub fn enable(&mut self) {
        if self.config.enable_inverted {
            self.enable_pin.set_low();
        } else {
            self.enable_pin.set_high();
        }
    }

    /// Disable the stepper driver
    pub fn disable(&mut self) {
        if self.config.enable_inverted {
            self.enable_pin.set_high();
        } else {
            self.enable_pin.set_low();
        }
    }

    /// Set direction
    pub fn set_direction(&mut self, clockwise: bool) {
        self.direction_cw = clockwise;
        if clockwise {
            self.dir_pin.set_low();
        } else {
            self.dir_pin.set_high();
        }
    }

    /// Get current direction
    pub fn direction(&self) -> bool {
        self.direction_cw
    }

    /// Set step frequency in Hz
    ///
    /// This directly updates the PIO clock divider.
    pub fn set_frequency(&mut self, freq_hz: u32) {
        let freq = freq_hz.min(MAX_STEP_FREQ_HZ);
        self.current_freq_hz = freq;

        if freq == 0 {
            self.stop();
            return;
        }

        let (int_div, frac_div) = calc_clock_divider(freq);

        // Convert to U24F8: integer in upper 24 bits, fractional in lower 8 bits
        let divider_bits = ((int_div as u32) << 8) | (frac_div as u32);
        self.sm.set_clock_divider(U24F8::from_bits(divider_bits));

        if !self.running {
            self.sm.set_enable(true);
            self.running = true;
        }
    }

    /// Set speed in RPM
    pub fn set_rpm(&mut self, rpm: u16) {
        let freq = (rpm as u32) * self.config.steps_per_rev / 60;
        self.set_frequency(freq);
    }

    /// Get current frequency in Hz
    pub fn current_freq(&self) -> u32 {
        self.current_freq_hz
    }

    /// Get current RPM
    pub fn current_rpm(&self) -> u16 {
        if self.config.steps_per_rev == 0 {
            return 0;
        }
        ((self.current_freq_hz * 60) / self.config.steps_per_rev) as u16
    }

    /// Stop pulse generation
    pub fn stop(&mut self) {
        self.sm.set_enable(false);
        self.running = false;
        self.current_freq_hz = 0;
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get steps per revolution
    pub fn steps_per_rev(&self) -> u32 {
        self.config.steps_per_rev
    }
}
