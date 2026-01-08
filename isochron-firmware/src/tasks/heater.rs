//! Heater control task
//!
//! Supports two control modes:
//! - Bang-bang: Simple on/off control with hysteresis
//! - PID: Time-proportioning PID control
//!
//! Also implements autotune using Åström-Hägglund relay method.

use defmt::*;
use embassy_rp::adc::{Adc, Async, Channel};
use embassy_rp::gpio::Output;
use embassy_time::{Duration, Ticker};

use isochron_core::config::HeaterControlMode;
use isochron_drivers::heater::{Fixed32, PidCoefficients};

use crate::channels::{
    AutotuneCommand, AutotuneFailure, AutotuneStatus, AUTOTUNE_CMD, AUTOTUNE_STATUS, HEATER_CMD,
    TEMP_READING,
};

/// Heater control configuration
#[derive(Clone)]
pub struct HeaterConfig {
    /// Control mode (BangBang or Pid)
    pub control_mode: HeaterControlMode,
    /// Maximum allowed temperature (°C)
    pub max_temp_c: i16,
    /// Hysteresis for bang-bang control (°C)
    pub hysteresis_c: i16,
    /// Pull-up resistor value in ohms
    pub pullup_ohms: u32,
    /// ADC resolution (12-bit = 4096)
    pub adc_max: u16,
    /// PID coefficients (value × 100)
    pub pid_kp_x100: i16,
    pub pid_ki_x100: i16,
    pub pid_kd_x100: i16,
    /// PWM period for PID time-proportioning (in ticks)
    pub pwm_period_ticks: u8,
}

impl Default for HeaterConfig {
    fn default() -> Self {
        Self {
            control_mode: HeaterControlMode::BangBang,
            max_temp_c: 55,
            hysteresis_c: 2,
            pullup_ohms: 4700,
            adc_max: 4096,
            pid_kp_x100: 0,
            pid_ki_x100: 0,
            pid_kd_x100: 0,
            pwm_period_ticks: 20, // 10 seconds at 500ms loop
        }
    }
}

/// NTC 100K thermistor temperature lookup table
/// Format: (resistance_ohms, temperature_c * 10)
const TEMP_TABLE: &[(u32, i16)] = &[
    (1_750_000, -200), // -20°C
    (1_000_000, -100), // -10°C
    (600_000, 0),      // 0°C
    (350_000, 100),    // 10°C
    (200_000, 200),    // 20°C
    (100_000, 250),    // 25°C (R0)
    (80_000, 300),     // 30°C
    (55_000, 400),     // 40°C
    (40_000, 450),     // 45°C
    (30_000, 500),     // 50°C
    (25_000, 550),     // 55°C
    (18_000, 600),     // 60°C
    (12_000, 700),     // 70°C
    (8_000, 800),      // 80°C
    (5_500, 900),      // 90°C
    (4_000, 1000),     // 100°C
];

/// Convert ADC reading to resistance
fn adc_to_resistance(adc_value: u16, pullup_ohms: u32, adc_max: u16) -> Option<u32> {
    // Check for open/short circuit
    if adc_value >= adc_max - 10 || adc_value < 10 {
        return None;
    }

    // R = pullup * adc / (adc_max - adc)
    let numerator = pullup_ohms as u64 * adc_value as u64;
    let denominator = (adc_max - adc_value) as u64;

    Some((numerator / denominator) as u32)
}

/// Convert resistance to temperature (in 0.1°C units)
fn resistance_to_temp_x10(resistance: u32) -> Option<i16> {
    // Check range
    if resistance > TEMP_TABLE[0].0 || resistance < TEMP_TABLE[TEMP_TABLE.len() - 1].0 {
        return None;
    }

    // Find and interpolate
    for i in 0..TEMP_TABLE.len() - 1 {
        let (r_high, t_low) = TEMP_TABLE[i];
        let (r_low, t_high) = TEMP_TABLE[i + 1];

        if resistance <= r_high && resistance >= r_low {
            let r_range = r_high - r_low;
            let t_range = t_high - t_low;
            let r_offset = r_high - resistance;

            let temp = t_low + (t_range as i32 * r_offset as i32 / r_range as i32) as i16;
            return Some(temp);
        }
    }

    None
}

/// Heater task mode
#[derive(Debug, Clone, Copy, PartialEq)]
enum TaskMode {
    /// Normal operation (bang-bang or PID based on config)
    Normal,
    /// Autotuning in progress
    Autotuning,
}

/// PID internal state
struct PidState {
    /// PID coefficients
    coefficients: PidCoefficients,
    /// Accumulated integral term
    integral: Fixed32,
    /// Previous error for derivative
    prev_error_x10: i16,
    /// Current duty cycle (0-255)
    duty: u8,
    /// Current PWM tick position
    pwm_tick: u8,
    /// Integral limit (×10)
    integral_limit_x10: i16,
    /// Deadband around setpoint (×10)
    deadband_x10: i16,
}

impl PidState {
    fn new(kp_x100: i16, ki_x100: i16, kd_x100: i16) -> Self {
        Self {
            coefficients: PidCoefficients::from_scaled_100(
                kp_x100 as i32,
                ki_x100 as i32,
                kd_x100 as i32,
            ),
            integral: Fixed32::ZERO,
            prev_error_x10: 0,
            duty: 0,
            pwm_tick: 0,
            integral_limit_x10: 200, // ±20.0°C
            deadband_x10: 2,         // 0.2°C
        }
    }

    fn reset(&mut self) {
        self.integral = Fixed32::ZERO;
        self.prev_error_x10 = 0;
        self.duty = 0;
        self.pwm_tick = 0;
    }

    fn update_coefficients(&mut self, kp_x100: i16, ki_x100: i16, kd_x100: i16) {
        self.coefficients =
            PidCoefficients::from_scaled_100(kp_x100 as i32, ki_x100 as i32, kd_x100 as i32);
        self.reset();
    }

    /// Calculate PID output and return duty cycle (0-255)
    fn calculate(&mut self, target_x10: i16, current_x10: i16) -> u8 {
        let error_x10 = target_x10 - current_x10;

        // Apply deadband
        let error_x10 = if error_x10.abs() <= self.deadband_x10 {
            0
        } else {
            error_x10
        };

        let error = Fixed32::from_int(error_x10);
        let coeffs = &self.coefficients;

        // Proportional term
        let p_term = coeffs.kp.mul(error);

        // Integral term with anti-windup
        let i_contribution = coeffs.ki.mul(error);
        self.integral = self.integral.saturating_add(i_contribution);

        let integral_limit = Fixed32::from_int(self.integral_limit_x10);
        self.integral = self.integral.clamp(-integral_limit, integral_limit);

        // Derivative term (on error)
        let d_error = error_x10 - self.prev_error_x10;
        let d_term = coeffs.kd.mul(Fixed32::from_int(d_error));
        self.prev_error_x10 = error_x10;

        // Sum terms
        let output = p_term.saturating_add(self.integral).saturating_add(d_term);

        // Scale to 0-255
        self.duty = output.to_int().clamp(0, 255) as u8;
        self.duty
    }

    /// Apply time-proportioning PWM, returns whether heater should be on
    fn apply_pwm(&mut self, duty: u8, period_ticks: u8) -> bool {
        self.pwm_tick = (self.pwm_tick + 1) % period_ticks;

        let threshold = if period_ticks > 0 {
            (self.pwm_tick as u16 * 255) / period_ticks as u16
        } else {
            0
        };

        (duty as u16) > threshold
    }
}

/// Autotune state
struct AutotuneState {
    /// Target temperature (×10)
    target_x10: i16,
    /// Maximum temperature (×10)
    max_temp_x10: i16,
    /// Hysteresis (×10)
    hysteresis_x10: i16,
    /// Tick counter
    tick_count: u32,
    /// Collected peaks (high temp, low temp alternating with tick)
    peaks: heapless::Vec<(i16, u32, bool), 24>, // (temp_x10, tick, is_high)
    /// Previous temperatures for peak detection
    prev_temp_x10: i16,
    prev_prev_temp_x10: i16,
    /// Current heater state
    heater_on: bool,
    /// Phase: 0 = heating to target, 1 = cycling
    phase: u8,
}

impl AutotuneState {
    fn new(target_x10: i16, max_temp_x10: i16) -> Self {
        Self {
            target_x10,
            max_temp_x10,
            hysteresis_x10: 5, // 0.5°C
            tick_count: 0,
            peaks: heapless::Vec::new(),
            prev_temp_x10: 0,
            prev_prev_temp_x10: 0,
            heater_on: true,
            phase: 0,
        }
    }

    /// Update autotune, returns (heater_should_be_on, finished, error)
    fn update(
        &mut self,
        temp_x10: i16,
    ) -> (bool, Option<Result<(i16, i16, i16), AutotuneFailure>>) {
        self.tick_count += 1;

        // Timeout check (20 minutes at 500ms = 2400 ticks)
        if self.tick_count >= 2400 {
            return (false, Some(Err(AutotuneFailure::Timeout)));
        }

        // Over-temp check
        if temp_x10 >= self.max_temp_x10 {
            return (false, Some(Err(AutotuneFailure::OverTemp)));
        }

        match self.phase {
            0 => {
                // Heating phase - wait to reach target zone
                let target_low = self.target_x10 - self.hysteresis_x10;
                if temp_x10 >= target_low {
                    self.phase = 1;
                    self.heater_on = false;
                    debug!("Autotune: reached target, starting oscillation");
                }
                (self.heater_on, None)
            }
            1 => {
                // Cycling phase - relay control with peak detection
                let target_high = self.target_x10 + self.hysteresis_x10;
                let target_low = self.target_x10 - self.hysteresis_x10;

                // Relay control
                if temp_x10 >= target_high && self.heater_on {
                    self.heater_on = false;
                } else if temp_x10 <= target_low && !self.heater_on {
                    self.heater_on = true;
                }

                // Peak detection (need 3 samples)
                if self.tick_count >= 3 {
                    // High peak
                    if self.prev_temp_x10 > self.prev_prev_temp_x10 && self.prev_temp_x10 > temp_x10
                    {
                        let _ = self
                            .peaks
                            .push((self.prev_temp_x10, self.tick_count - 1, true));
                        debug!(
                            "Autotune: high peak {} at tick {}",
                            self.prev_temp_x10,
                            self.tick_count - 1
                        );
                    }
                    // Low peak
                    else if self.prev_temp_x10 < self.prev_prev_temp_x10
                        && self.prev_temp_x10 < temp_x10
                    {
                        let _ = self
                            .peaks
                            .push((self.prev_temp_x10, self.tick_count - 1, false));
                        debug!(
                            "Autotune: low peak {} at tick {}",
                            self.prev_temp_x10,
                            self.tick_count - 1
                        );
                    }
                }

                self.prev_prev_temp_x10 = self.prev_temp_x10;
                self.prev_temp_x10 = temp_x10;

                // Check if we have enough peaks (12+)
                if self.peaks.len() >= 12 {
                    match self.calculate_pid() {
                        Some((kp, ki, kd)) => {
                            return (false, Some(Ok((kp, ki, kd))));
                        }
                        None => {
                            return (false, Some(Err(AutotuneFailure::NoOscillation)));
                        }
                    }
                }

                (self.heater_on, None)
            }
            _ => (false, None),
        }
    }

    /// Calculate PID coefficients from peaks using Ziegler-Nichols
    fn calculate_pid(&self) -> Option<(i16, i16, i16)> {
        // Separate high and low peaks
        let high_peaks: heapless::Vec<&(i16, u32, bool), 24> =
            self.peaks.iter().filter(|p| p.2).collect();
        let low_peaks: heapless::Vec<&(i16, u32, bool), 24> =
            self.peaks.iter().filter(|p| !p.2).collect();

        if high_peaks.len() < 2 || low_peaks.len() < 2 {
            return None;
        }

        // Calculate average amplitude
        let avg_high: i32 =
            high_peaks.iter().map(|p| p.0 as i32).sum::<i32>() / high_peaks.len() as i32;
        let avg_low: i32 =
            low_peaks.iter().map(|p| p.0 as i32).sum::<i32>() / low_peaks.len() as i32;
        let amplitude = (avg_high - avg_low) / 2;

        if amplitude < 5 {
            return None;
        }

        // Calculate average period
        let mut period_sum: u32 = 0;
        let mut period_count: u32 = 0;

        for window in high_peaks.windows(2) {
            period_sum += window[1].1 - window[0].1;
            period_count += 1;
        }
        for window in low_peaks.windows(2) {
            period_sum += window[1].1 - window[0].1;
            period_count += 1;
        }

        if period_count == 0 {
            return None;
        }

        let tu = period_sum / period_count;
        if tu < 4 {
            return None;
        }

        // Ku = 4 * d / (π * a), d = 255, using π ≈ 314/100
        let ku_x100: i32 = (4 * 255 * 10000) / (314 * amplitude);

        // Ziegler-Nichols PID:
        // Kp = 0.6 * Ku
        // Ki = 1.2 * Ku / Tu
        // Kd = 0.075 * Ku * Tu
        let kp_x100 = (60 * ku_x100) / 100;
        let ki_x100 = (120 * ku_x100) / (100 * tu as i32);
        let kd_x100 = (75 * ku_x100 * tu as i32) / 10000;

        info!(
            "Autotune complete: Ku={}, Tu={}, Kp={}, Ki={}, Kd={}",
            ku_x100, tu, kp_x100, ki_x100, kd_x100
        );

        Some((
            kp_x100.clamp(0, i16::MAX as i32) as i16,
            ki_x100.clamp(0, i16::MAX as i32) as i16,
            kd_x100.clamp(0, i16::MAX as i32) as i16,
        ))
    }
}

/// Heater control task
///
/// Reads thermistor via ADC and controls heater GPIO with either
/// bang-bang or PID control logic.
#[embassy_executor::task]
pub async fn heater_task(
    mut adc: Adc<'static, Async>,
    mut therm_channel: Channel<'static>,
    mut heater_pin: Output<'static>,
    config: HeaterConfig,
) {
    info!("Heater task started (mode: {:?})", config.control_mode);

    // Start with heater off
    heater_pin.set_low();

    // State
    let mut target_temp_c: Option<i16> = None;
    let mut heater_on = false;
    let mut mode = TaskMode::Normal;

    // PID state (initialized even for bang-bang, used if autotune completes)
    let mut pid_state = PidState::new(config.pid_kp_x100, config.pid_ki_x100, config.pid_kd_x100);

    // Autotune state
    let mut autotune_state: Option<AutotuneState> = None;
    let mut autotune_progress_tick: u32 = 0;

    // Control loop ticker (update every 500ms)
    let mut ticker = Ticker::every(Duration::from_millis(500));

    loop {
        // Check for autotune command (non-blocking)
        if let Some(cmd) = AUTOTUNE_CMD.try_take() {
            match cmd {
                AutotuneCommand::Start { target_x10 } => {
                    info!("Starting autotune at target {}°C", target_x10 / 10);
                    mode = TaskMode::Autotuning;
                    autotune_state = Some(AutotuneState::new(
                        target_x10,
                        config.max_temp_c as i16 * 10,
                    ));
                    autotune_progress_tick = 0;
                    heater_pin.set_high();
                    heater_on = true;
                    AUTOTUNE_STATUS.signal(AutotuneStatus::Started);
                }
                AutotuneCommand::Cancel => {
                    if mode == TaskMode::Autotuning {
                        info!("Autotune cancelled");
                        mode = TaskMode::Normal;
                        autotune_state = None;
                        heater_pin.set_low();
                        heater_on = false;
                        AUTOTUNE_STATUS.signal(AutotuneStatus::Failed(AutotuneFailure::Cancelled));
                    }
                }
            }
        }

        // Check for heater command (only in normal mode)
        if mode == TaskMode::Normal {
            if let Some(cmd) = HEATER_CMD.try_take() {
                target_temp_c = cmd.target_temp_c;
                if target_temp_c.is_none() {
                    heater_pin.set_low();
                    heater_on = false;
                    pid_state.reset();
                    debug!("Heater disabled");
                } else {
                    debug!("Heater target: {}°C", cmd.target_temp_c.unwrap());
                }
            }
        }

        // Read temperature
        match adc.read(&mut therm_channel).await {
            Ok(adc_value) => {
                if let Some(resistance) =
                    adc_to_resistance(adc_value, config.pullup_ohms, config.adc_max)
                {
                    if let Some(temp_x10) = resistance_to_temp_x10(resistance) {
                        let temp_c = temp_x10 / 10;
                        trace!("Temperature: {}.{}°C", temp_c, (temp_x10 % 10).abs());

                        // Signal temperature to controller
                        TEMP_READING.signal(Some(temp_x10));

                        match mode {
                            TaskMode::Normal => {
                                if let Some(target) = target_temp_c {
                                    // Safety check
                                    if temp_c >= config.max_temp_c {
                                        if heater_on {
                                            heater_pin.set_low();
                                            heater_on = false;
                                            warn!("Max temperature reached, heater off");
                                        }
                                    } else {
                                        // Apply control based on mode
                                        let should_be_on = match config.control_mode {
                                            HeaterControlMode::BangBang => apply_bang_bang(
                                                temp_c,
                                                target,
                                                config.hysteresis_c,
                                                heater_on,
                                            ),
                                            HeaterControlMode::Pid => {
                                                let target_x10 = target * 10;
                                                let duty =
                                                    pid_state.calculate(target_x10, temp_x10);
                                                pid_state.apply_pwm(duty, config.pwm_period_ticks)
                                            }
                                        };

                                        if should_be_on != heater_on {
                                            if should_be_on {
                                                heater_pin.set_high();
                                            } else {
                                                heater_pin.set_low();
                                            }
                                            heater_on = should_be_on;
                                        }
                                    }
                                }
                            }
                            TaskMode::Autotuning => {
                                if let Some(ref mut state) = autotune_state {
                                    let (should_be_on, result) = state.update(temp_x10);

                                    // Update heater
                                    if should_be_on != heater_on {
                                        if should_be_on {
                                            heater_pin.set_high();
                                        } else {
                                            heater_pin.set_low();
                                        }
                                        heater_on = should_be_on;
                                    }

                                    // Send progress every 10 ticks
                                    autotune_progress_tick += 1;
                                    if autotune_progress_tick >= 10 {
                                        autotune_progress_tick = 0;
                                        AUTOTUNE_STATUS.signal(AutotuneStatus::Progress {
                                            peaks: state.peaks.len() as u8,
                                            ticks: state.tick_count,
                                        });
                                    }

                                    // Handle completion
                                    if let Some(result) = result {
                                        match result {
                                            Ok((kp, ki, kd)) => {
                                                info!("Autotune complete");
                                                // Update PID state with new coefficients
                                                pid_state.update_coefficients(kp, ki, kd);
                                                AUTOTUNE_STATUS.signal(AutotuneStatus::Complete {
                                                    kp_x100: kp,
                                                    ki_x100: ki,
                                                    kd_x100: kd,
                                                });
                                            }
                                            Err(e) => {
                                                warn!("Autotune failed: {:?}", e);
                                                AUTOTUNE_STATUS.signal(AutotuneStatus::Failed(e));
                                            }
                                        }
                                        mode = TaskMode::Normal;
                                        autotune_state = None;
                                        heater_pin.set_low();
                                        heater_on = false;
                                    }
                                }
                            }
                        }
                    } else {
                        warn!("Temperature out of range");
                        TEMP_READING.signal(None);
                        handle_sensor_fault(
                            &mut heater_pin,
                            &mut heater_on,
                            &mut mode,
                            &mut autotune_state,
                        );
                    }
                } else {
                    warn!("Thermistor fault (open/short)");
                    TEMP_READING.signal(None);
                    handle_sensor_fault(
                        &mut heater_pin,
                        &mut heater_on,
                        &mut mode,
                        &mut autotune_state,
                    );
                }
            }
            Err(_) => {
                warn!("ADC read error");
                TEMP_READING.signal(None);
                handle_sensor_fault(
                    &mut heater_pin,
                    &mut heater_on,
                    &mut mode,
                    &mut autotune_state,
                );
            }
        }

        ticker.next().await;
    }
}

/// Apply bang-bang control logic
fn apply_bang_bang(temp_c: i16, target: i16, hysteresis: i16, currently_on: bool) -> bool {
    let low_threshold = target - hysteresis;
    let high_threshold = target + hysteresis;

    if temp_c < low_threshold {
        true
    } else if temp_c > high_threshold {
        false
    } else {
        currently_on
    }
}

/// Handle sensor fault - turn off heater and abort autotune
fn handle_sensor_fault(
    heater_pin: &mut Output<'static>,
    heater_on: &mut bool,
    mode: &mut TaskMode,
    autotune_state: &mut Option<AutotuneState>,
) {
    if *heater_on {
        heater_pin.set_low();
        *heater_on = false;
    }

    if *mode == TaskMode::Autotuning {
        AUTOTUNE_STATUS.signal(AutotuneStatus::Failed(AutotuneFailure::SensorFault));
        *mode = TaskMode::Normal;
        *autotune_state = None;
    }
}
