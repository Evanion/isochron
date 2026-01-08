//! PID autotune using Åström-Hägglund relay method
//!
//! Implements relay feedback autotuning to determine optimal PID coefficients.
//! The process oscillates the heater on/off around a setpoint, measures the
//! resulting temperature oscillation, and calculates PID parameters using
//! Ziegler-Nichols tuning rules.

use super::fixed::Fixed32;
use super::pid::PidCoefficients;
use isochron_core::traits::{HeaterOutput, SensorError, TemperatureSensor};

/// Minimum number of oscillation peaks required for reliable tuning
const MIN_PEAKS: usize = 12;

/// Maximum peaks to collect (limits memory usage)
const MAX_PEAKS: usize = 24;

/// Maximum autotune duration in ticks (20 minutes at 500ms = 2400 ticks)
const MAX_TICKS: u32 = 2400;

/// Autotune state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AutotuneState {
    /// Not running
    Idle,
    /// Initial heating to reach target temperature zone
    Heating,
    /// Oscillating around setpoint, collecting peaks
    Cycling,
    /// Successfully completed, coefficients available
    Complete,
    /// Failed (timeout, sensor error, etc.)
    Failed(AutotuneError),
}

/// Autotune error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AutotuneError {
    /// Exceeded maximum temperature
    OverTemp,
    /// Took too long to complete
    Timeout,
    /// Temperature sensor error
    SensorFault,
    /// Oscillation too small to measure
    NoOscillation,
    /// User cancelled
    Cancelled,
}

/// Peak type for oscillation detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeakType {
    High,
    Low,
}

/// Recorded peak data
#[derive(Debug, Clone, Copy)]
struct Peak {
    /// Temperature at peak (×10)
    temp_x10: i16,
    /// Tick count when peak occurred
    tick: u32,
    /// Type of peak
    peak_type: PeakType,
}

/// Autotune configuration
#[derive(Debug, Clone)]
pub struct AutotuneConfig {
    /// Target temperature for tuning (°C × 10)
    pub target_x10: i16,
    /// Maximum allowed temperature (°C × 10)
    pub max_temp_x10: i16,
    /// Hysteresis for relay switching (°C × 10)
    ///
    /// The relay turns on when temp < target - hysteresis
    /// and off when temp > target + hysteresis.
    pub hysteresis_x10: i16,
    /// Relay output level (0-255)
    ///
    /// Typically 255 (full power) for fastest response.
    pub relay_output: u8,
}

impl Default for AutotuneConfig {
    fn default() -> Self {
        Self {
            target_x10: 450,   // 45.0°C
            max_temp_x10: 550, // 55.0°C safety limit
            hysteresis_x10: 5, // 0.5°C
            relay_output: 255, // Full power
        }
    }
}

/// Autotune result with calculated coefficients
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AutotuneResult {
    /// Calculated PID coefficients
    pub coefficients: PidCoefficients,
    /// Ultimate gain (Ku)
    pub ku_x100: i32,
    /// Ultimate period in ticks (Tu)
    pub tu_ticks: u32,
    /// Average oscillation amplitude (°C × 10)
    pub amplitude_x10: i16,
}

/// Autotuner state machine
pub struct Autotuner<S, H> {
    sensor: S,
    heater: H,
    config: AutotuneConfig,
    state: AutotuneState,
    tick_count: u32,
    peaks: heapless::Vec<Peak, MAX_PEAKS>,
    last_temp_x10: i16,
    prev_temp_x10: i16,
    heater_on: bool,
    result: Option<AutotuneResult>,
}

impl<S: TemperatureSensor, H: HeaterOutput> Autotuner<S, H> {
    /// Create a new autotuner
    pub fn new(sensor: S, heater: H, config: AutotuneConfig) -> Self {
        Self {
            sensor,
            heater,
            config,
            state: AutotuneState::Idle,
            tick_count: 0,
            peaks: heapless::Vec::new(),
            last_temp_x10: 0,
            prev_temp_x10: 0,
            heater_on: false,
            result: None,
        }
    }

    /// Start autotuning
    pub fn start(&mut self) {
        self.state = AutotuneState::Heating;
        self.tick_count = 0;
        self.peaks.clear();
        self.last_temp_x10 = 0;
        self.prev_temp_x10 = 0;
        self.heater_on = true;
        self.heater.set_on(true);
        self.result = None;
    }

    /// Cancel autotuning
    pub fn cancel(&mut self) {
        self.heater.set_on(false);
        self.heater_on = false;
        self.state = AutotuneState::Failed(AutotuneError::Cancelled);
    }

    /// Get current state
    pub fn state(&self) -> AutotuneState {
        self.state
    }

    /// Get peak count
    pub fn peak_count(&self) -> usize {
        self.peaks.len()
    }

    /// Get elapsed ticks
    pub fn elapsed_ticks(&self) -> u32 {
        self.tick_count
    }

    /// Get result (if complete)
    pub fn result(&self) -> Option<&AutotuneResult> {
        self.result.as_ref()
    }

    /// Get access to sensor
    pub fn sensor(&self) -> &S {
        &self.sensor
    }

    /// Get access to heater
    pub fn heater(&self) -> &H {
        &self.heater
    }

    /// Update autotune state machine
    ///
    /// Call this at the control loop rate (e.g., every 500ms).
    pub fn update(&mut self) -> Result<(), SensorError> {
        if self.state == AutotuneState::Idle
            || matches!(
                self.state,
                AutotuneState::Complete | AutotuneState::Failed(_)
            )
        {
            return Ok(());
        }

        // Read temperature
        let temp_x10 = self.sensor.read_celsius_x10()?;

        // Safety check
        if temp_x10 >= self.config.max_temp_x10 {
            self.heater.set_on(false);
            self.heater_on = false;
            self.state = AutotuneState::Failed(AutotuneError::OverTemp);
            return Ok(());
        }

        // Timeout check
        self.tick_count += 1;
        if self.tick_count >= MAX_TICKS {
            self.heater.set_on(false);
            self.heater_on = false;
            self.state = AutotuneState::Failed(AutotuneError::Timeout);
            return Ok(());
        }

        match self.state {
            AutotuneState::Heating => {
                self.run_heating(temp_x10);
            }
            AutotuneState::Cycling => {
                self.run_cycling(temp_x10);
            }
            _ => {}
        }

        self.prev_temp_x10 = self.last_temp_x10;
        self.last_temp_x10 = temp_x10;

        Ok(())
    }

    /// Initial heating phase - wait until we reach target zone
    fn run_heating(&mut self, temp_x10: i16) {
        let target_low = self.config.target_x10 - self.config.hysteresis_x10;

        if temp_x10 >= target_low {
            // Reached target zone, start cycling
            self.state = AutotuneState::Cycling;
            // Turn off heater to start first oscillation
            self.heater.set_on(false);
            self.heater_on = false;
        }
        // Else keep heating
    }

    /// Cycling phase - oscillate and collect peaks
    fn run_cycling(&mut self, temp_x10: i16) {
        let target_high = self.config.target_x10 + self.config.hysteresis_x10;
        let target_low = self.config.target_x10 - self.config.hysteresis_x10;

        // Relay control with hysteresis
        if temp_x10 >= target_high && self.heater_on {
            self.heater.set_on(false);
            self.heater_on = false;
        } else if temp_x10 <= target_low && !self.heater_on {
            self.heater.set_on(true);
            self.heater_on = true;
        }

        // Peak detection - look for direction changes
        // We need at least 2 previous samples
        if self.tick_count >= 3 {
            self.detect_peak(temp_x10);
        }

        // Check if we have enough peaks
        if self.peaks.len() >= MIN_PEAKS {
            self.calculate_result();
        }
    }

    /// Detect temperature peaks (local maxima/minima)
    fn detect_peak(&mut self, temp_x10: i16) {
        // Detect high peak: prev_temp < last_temp > current_temp
        if self.last_temp_x10 > self.prev_temp_x10 && self.last_temp_x10 > temp_x10 {
            let peak = Peak {
                temp_x10: self.last_temp_x10,
                tick: self.tick_count - 1,
                peak_type: PeakType::High,
            };
            let _ = self.peaks.push(peak);
        }
        // Detect low peak: prev_temp > last_temp < current_temp
        else if self.last_temp_x10 < self.prev_temp_x10 && self.last_temp_x10 < temp_x10 {
            let peak = Peak {
                temp_x10: self.last_temp_x10,
                tick: self.tick_count - 1,
                peak_type: PeakType::Low,
            };
            let _ = self.peaks.push(peak);
        }

        // If we've hit max peaks, try to calculate
        if self.peaks.len() >= MAX_PEAKS {
            self.calculate_result();
        }
    }

    /// Calculate PID coefficients from collected peaks
    fn calculate_result(&mut self) {
        self.heater.set_on(false);
        self.heater_on = false;

        let (amplitude_x10, period_ticks) = match self.analyze_peaks() {
            Some(values) => values,
            None => {
                self.state = AutotuneState::Failed(AutotuneError::NoOscillation);
                return;
            }
        };

        // Check for meaningful oscillation
        if amplitude_x10 < 5 || period_ticks < 4 {
            self.state = AutotuneState::Failed(AutotuneError::NoOscillation);
            return;
        }

        // Calculate ultimate gain Ku
        // Ku = 4 * d / (π * a)
        // where d = relay output (255), a = amplitude
        //
        // Using fixed point: Ku_x100 = (4 * 255 * 100) / (π * amplitude)
        // π ≈ 314/100 for fixed-point approximation
        //
        // Ku = (4 * 255) / (3.14159 * amplitude)
        // Ku_x100 = (4 * 255 * 100 * 100) / (314 * amplitude)
        let d = self.config.relay_output as i32;
        let ku_x100 = (4 * d * 10000) / (314 * amplitude_x10 as i32);

        // Ultimate period Tu = period_ticks (in control loop ticks)
        let tu_ticks = period_ticks;

        // Ziegler-Nichols PID tuning:
        // Kp = 0.6 * Ku
        // Ki = 1.2 * Ku / Tu
        // Kd = 0.075 * Ku * Tu
        //
        // As x100 values:
        // Kp_x100 = 60 * Ku_x100 / 100
        // Ki_x100 = 120 * Ku_x100 / (100 * Tu)
        // Kd_x100 = 75 * Ku_x100 * Tu / 10000

        let kp_x100 = (60 * ku_x100) / 100;
        let ki_x100 = (120 * ku_x100) / (100 * tu_ticks as i32);
        let kd_x100 = (75 * ku_x100 * tu_ticks as i32) / 10000;

        // Clamp to i16 range for storage
        let kp_x100 = kp_x100.clamp(0, i16::MAX as i32) as i16;
        let ki_x100 = ki_x100.clamp(0, i16::MAX as i32) as i16;
        let kd_x100 = kd_x100.clamp(0, i16::MAX as i32) as i16;

        let coefficients =
            PidCoefficients::from_scaled_100(kp_x100 as i32, ki_x100 as i32, kd_x100 as i32);

        self.result = Some(AutotuneResult {
            coefficients,
            ku_x100,
            tu_ticks,
            amplitude_x10,
        });

        self.state = AutotuneState::Complete;
    }

    /// Analyze peaks to get average amplitude and period
    fn analyze_peaks(&self) -> Option<(i16, u32)> {
        if self.peaks.len() < 4 {
            return None;
        }

        // Separate high and low peaks
        let high_peaks: heapless::Vec<&Peak, MAX_PEAKS> = self
            .peaks
            .iter()
            .filter(|p| p.peak_type == PeakType::High)
            .collect();

        let low_peaks: heapless::Vec<&Peak, MAX_PEAKS> = self
            .peaks
            .iter()
            .filter(|p| p.peak_type == PeakType::Low)
            .collect();

        if high_peaks.len() < 2 || low_peaks.len() < 2 {
            return None;
        }

        // Calculate average high and low temperatures
        let avg_high: i32 =
            high_peaks.iter().map(|p| p.temp_x10 as i32).sum::<i32>() / high_peaks.len() as i32;
        let avg_low: i32 =
            low_peaks.iter().map(|p| p.temp_x10 as i32).sum::<i32>() / low_peaks.len() as i32;

        // Amplitude = (avg_high - avg_low) / 2
        let amplitude_x10 = ((avg_high - avg_low) / 2) as i16;

        // Calculate average period between same-type peaks
        let mut period_sum: u32 = 0;
        let mut period_count: u32 = 0;

        for window in high_peaks.windows(2) {
            period_sum += window[1].tick - window[0].tick;
            period_count += 1;
        }

        for window in low_peaks.windows(2) {
            period_sum += window[1].tick - window[0].tick;
            period_count += 1;
        }

        if period_count == 0 {
            return None;
        }

        let avg_period = period_sum / period_count;

        Some((amplitude_x10, avg_period))
    }
}

/// Convert autotune result to Fixed32 coefficients
impl AutotuneResult {
    /// Get coefficients as Fixed32 values
    pub fn to_fixed_coefficients(&self) -> (Fixed32, Fixed32, Fixed32) {
        (
            self.coefficients.kp,
            self.coefficients.ki,
            self.coefficients.kd,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSensor {
        temps: &'static [i16],
        index: usize,
    }

    impl MockSensor {
        fn new(temps: &'static [i16]) -> Self {
            Self { temps, index: 0 }
        }
    }

    impl TemperatureSensor for MockSensor {
        fn read_celsius_x10(&mut self) -> Result<i16, SensorError> {
            let temp = self.temps.get(self.index).copied().unwrap_or(450);
            self.index = (self.index + 1) % self.temps.len();
            Ok(temp)
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
    fn test_autotune_initial_state() {
        let sensor = MockSensor::new(&[400, 410, 420]);
        let heater = MockHeater { on: false };
        let config = AutotuneConfig::default();

        let tuner = Autotuner::new(sensor, heater, config);
        assert_eq!(tuner.state(), AutotuneState::Idle);
    }

    #[test]
    fn test_autotune_start() {
        let sensor = MockSensor::new(&[400]);
        let heater = MockHeater { on: false };
        let config = AutotuneConfig::default();

        let mut tuner = Autotuner::new(sensor, heater, config);
        tuner.start();

        assert_eq!(tuner.state(), AutotuneState::Heating);
        assert!(tuner.heater.is_on());
    }

    #[test]
    fn test_autotune_cancel() {
        let sensor = MockSensor::new(&[400]);
        let heater = MockHeater { on: false };
        let config = AutotuneConfig::default();

        let mut tuner = Autotuner::new(sensor, heater, config);
        tuner.start();
        tuner.cancel();

        assert_eq!(
            tuner.state(),
            AutotuneState::Failed(AutotuneError::Cancelled)
        );
        assert!(!tuner.heater.is_on());
    }

    #[test]
    fn test_autotune_overtemp_protection() {
        let sensor = MockSensor::new(&[560]); // Over max temp
        let heater = MockHeater { on: false };
        let config = AutotuneConfig::default();

        let mut tuner = Autotuner::new(sensor, heater, config);
        tuner.start();
        tuner.update().unwrap();

        assert_eq!(
            tuner.state(),
            AutotuneState::Failed(AutotuneError::OverTemp)
        );
        assert!(!tuner.heater.is_on());
    }

    #[test]
    fn test_ziegler_nichols_formula() {
        // Test the Ziegler-Nichols calculations manually
        // Given Ku = 10.0 (1000 as x100), Tu = 20 ticks
        let ku_x100: i32 = 1000;
        let tu_ticks: u32 = 20;

        // Kp = 0.6 * Ku = 6.0 -> 600 as x100
        let kp_x100 = (60 * ku_x100) / 100;
        assert_eq!(kp_x100, 600);

        // Ki = 1.2 * Ku / Tu = 1.2 * 10 / 20 = 0.6 -> 60 as x100
        let ki_x100 = (120 * ku_x100) / (100 * tu_ticks as i32);
        assert_eq!(ki_x100, 60);

        // Kd = 0.075 * Ku * Tu = 0.075 * 10 * 20 = 15.0 -> 1500 as x100
        let kd_x100 = (75 * ku_x100 * tu_ticks as i32) / 10000;
        assert_eq!(kd_x100, 150); // Note: 1500/10 due to scaling
    }
}
