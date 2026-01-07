//! Heater control task
//!
//! Implements bang-bang temperature control using ADC thermistor reading
//! and GPIO heater output.

use defmt::*;
use embassy_rp::adc::{Adc, Async, Channel};
use embassy_rp::gpio::Output;
use embassy_time::{Duration, Ticker};

use crate::channels::{HEATER_CMD, TEMP_READING};

/// Heater control configuration
pub struct HeaterConfig {
    /// Maximum allowed temperature (°C)
    pub max_temp_c: i16,
    /// Hysteresis (°C)
    pub hysteresis_c: i16,
    /// Pull-up resistor value in ohms
    pub pullup_ohms: u32,
    /// ADC resolution (12-bit = 4096)
    pub adc_max: u16,
}

impl Default for HeaterConfig {
    fn default() -> Self {
        Self {
            max_temp_c: 55,
            hysteresis_c: 2,
            pullup_ohms: 4700,
            adc_max: 4096,
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

/// Heater control task
///
/// Reads thermistor via ADC and controls heater GPIO with bang-bang logic.
#[embassy_executor::task]
pub async fn heater_task(
    mut adc: Adc<'static, Async>,
    mut therm_channel: Channel<'static>,
    mut heater_pin: Output<'static>,
    config: HeaterConfig,
) {
    info!("Heater task started");

    // Start with heater off
    heater_pin.set_low();

    // State
    let mut target_temp_c: Option<i16> = None;
    let mut heater_on = false;
    let mut _last_temp_x10: Option<i16> = None; // For future use (trend analysis)

    // Control loop ticker (update every 500ms)
    let mut ticker = Ticker::every(Duration::from_millis(500));

    loop {
        // Check for new heater command (non-blocking)
        if let Some(cmd) = HEATER_CMD.try_take() {
            target_temp_c = cmd.target_temp_c;
            if target_temp_c.is_none() {
                // Heater off
                heater_pin.set_low();
                heater_on = false;
                debug!("Heater disabled");
            } else {
                debug!("Heater target: {}°C", cmd.target_temp_c.unwrap());
            }
        }

        // Read temperature
        match adc.read(&mut therm_channel).await {
            Ok(adc_value) => {
                if let Some(resistance) =
                    adc_to_resistance(adc_value, config.pullup_ohms, config.adc_max)
                {
                    if let Some(temp_x10) = resistance_to_temp_x10(resistance) {
                        _last_temp_x10 = Some(temp_x10);
                        let temp_c = temp_x10 / 10;

                        trace!("Temperature: {}.{}°C", temp_c, (temp_x10 % 10).abs());

                        // Signal temperature to controller for safety monitoring
                        TEMP_READING.signal(Some(temp_x10));

                        // Apply bang-bang control if target is set
                        if let Some(target) = target_temp_c {
                            // Safety: never exceed max temperature
                            if temp_c >= config.max_temp_c {
                                if heater_on {
                                    heater_pin.set_low();
                                    heater_on = false;
                                    warn!("Max temperature reached, heater off");
                                }
                            } else {
                                // Bang-bang with hysteresis
                                let low_threshold = target - config.hysteresis_c;
                                let high_threshold = target + config.hysteresis_c;

                                if temp_c < low_threshold && !heater_on {
                                    heater_pin.set_high();
                                    heater_on = true;
                                    debug!("Heater ON (temp {}°C < {}°C)", temp_c, low_threshold);
                                } else if temp_c > high_threshold && heater_on {
                                    heater_pin.set_low();
                                    heater_on = false;
                                    debug!("Heater OFF (temp {}°C > {}°C)", temp_c, high_threshold);
                                }
                            }
                        }
                    } else {
                        warn!("Temperature out of range");
                        // Signal out-of-range as sensor fault
                        TEMP_READING.signal(None);
                    }
                } else {
                    warn!("Thermistor fault (open/short)");
                    // Signal sensor fault to controller
                    TEMP_READING.signal(None);
                    // Safety: turn off heater on sensor fault
                    if heater_on {
                        heater_pin.set_low();
                        heater_on = false;
                    }
                }
            }
            Err(_) => {
                warn!("ADC read error");
                // Signal read error as sensor fault
                TEMP_READING.signal(None);
            }
        }

        ticker.next().await;
    }
}
