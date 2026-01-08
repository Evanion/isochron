//! NTC 100K thermistor sensor
//!
//! Common thermistor used in 3D printing for temperature sensing.
//! Uses a lookup table for integer-only temperature calculation.

use isochron_core::traits::{SensorError, TemperatureSensor};

/// NTC 100K thermistor temperature lookup table
///
/// Table format: (resistance_ohms, temperature_x10)
/// Generated using beta equation with:
/// - R0 = 100,000 ohms at T0 = 25°C
/// - Beta = 3950K (typical for 100K NTC)
///
/// Temperature range: -20°C to 150°C
const TEMP_TABLE: &[(u32, i16)] = &[
    (1_750_000, -200), // -20°C (very cold)
    (1_000_000, -100), // -10°C
    (600_000, 0),      // 0°C
    (350_000, 100),    // 10°C
    (200_000, 200),    // 20°C
    (100_000, 250),    // 25°C (R0)
    (80_000, 300),     // 30°C
    (55_000, 400),     // 40°C
    (40_000, 450),     // 45°C (typical target)
    (30_000, 500),     // 50°C
    (25_000, 550),     // 55°C (max safe)
    (18_000, 600),     // 60°C
    (12_000, 700),     // 70°C
    (8_000, 800),      // 80°C
    (5_500, 900),      // 90°C
    (4_000, 1000),     // 100°C
    (2_000, 1200),     // 120°C
    (1_000, 1500),     // 150°C (danger)
];

/// ADC reading trait for platform abstraction
pub trait AdcReader {
    /// Read ADC value (12-bit, 0-4095)
    #[allow(clippy::result_unit_err)]
    fn read(&mut self) -> Result<u16, ()>;
}

/// NTC 100K thermistor with B=3950
///
/// Uses lookup table with linear interpolation for temperature calculation.
pub struct Ntc100kSensor<ADC> {
    adc: ADC,
    /// ADC reference voltage in mV (stored for potential future use)
    #[allow(dead_code)]
    vref_mv: u16,
    /// Pull-up resistor value in ohms
    pullup_ohms: u32,
    /// ADC resolution (typically 4096 for 12-bit)
    adc_max: u16,
}

impl<ADC> Ntc100kSensor<ADC> {
    /// Create a new NTC sensor
    ///
    /// # Arguments
    /// - `adc`: ADC channel for reading thermistor
    /// - `vref_mv`: Reference voltage in millivolts (typically 3300)
    /// - `pullup_ohms`: Pull-up resistor value (typically 4700 for 3.3V systems)
    pub fn new(adc: ADC, vref_mv: u16, pullup_ohms: u32) -> Self {
        Self {
            adc,
            vref_mv,
            pullup_ohms,
            adc_max: 4096, // 12-bit ADC
        }
    }

    /// Convert ADC reading to resistance
    ///
    /// Circuit: VCC -- pullup -- ADC_PIN -- NTC -- GND
    /// R_ntc = R_pullup * adc_value / (adc_max - adc_value)
    pub fn adc_to_resistance(&self, adc_value: u16) -> Result<u32, SensorError> {
        // Check for open circuit (ADC at max)
        if adc_value >= self.adc_max - 10 {
            return Err(SensorError::OpenCircuit);
        }

        // Check for short circuit (ADC at zero)
        if adc_value < 10 {
            return Err(SensorError::ShortCircuit);
        }

        // Calculate resistance
        // R = pullup * adc / (adc_max - adc)
        let numerator = self.pullup_ohms as u64 * adc_value as u64;
        let denominator = (self.adc_max - adc_value) as u64;

        Ok((numerator / denominator) as u32)
    }

    /// Calculate temperature from resistance using lookup table
    ///
    /// Returns temperature in 0.1°C units (e.g., 250 = 25.0°C).
    /// Uses linear interpolation between table entries.
    pub fn resistance_to_temp_x10(resistance: u32) -> Result<i16, SensorError> {
        // Check for out-of-range values
        if resistance > TEMP_TABLE[0].0 {
            // Resistance too high = too cold or open
            return Err(SensorError::OutOfRange);
        }

        if resistance < TEMP_TABLE[TEMP_TABLE.len() - 1].0 {
            // Resistance too low = too hot or shorted
            return Err(SensorError::OutOfRange);
        }

        // Find the two table entries to interpolate between
        // Table is sorted by decreasing resistance (increasing temperature)
        for i in 0..TEMP_TABLE.len() - 1 {
            let (r_high, t_low) = TEMP_TABLE[i];
            let (r_low, t_high) = TEMP_TABLE[i + 1];

            if resistance <= r_high && resistance >= r_low {
                // Linear interpolation
                // temp = t_low + (t_high - t_low) * (r_high - r) / (r_high - r_low)
                let r_range = r_high - r_low;
                let t_range = t_high - t_low;
                let r_offset = r_high - resistance;

                let temp = t_low + (t_range as i32 * r_offset as i32 / r_range as i32) as i16;
                return Ok(temp);
            }
        }

        // Shouldn't reach here, but just in case
        Err(SensorError::OutOfRange)
    }
}

impl<ADC: AdcReader> TemperatureSensor for Ntc100kSensor<ADC> {
    fn read_celsius_x10(&mut self) -> Result<i16, SensorError> {
        // Read ADC value
        let adc_value = self.adc.read().map_err(|_| SensorError::ConversionError)?;

        // Convert to resistance
        let resistance = self.adc_to_resistance(adc_value)?;

        // Convert to temperature
        Self::resistance_to_temp_x10(resistance)
    }
}

/// Dummy ADC for testing (returns a fixed value)
#[cfg(test)]
pub struct DummyAdc(pub u16);

#[cfg(test)]
impl AdcReader for DummyAdc {
    fn read(&mut self) -> Result<u16, ()> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resistance_to_temp() {
        // 100K ohms = 25°C (reference point)
        let temp = Ntc100kSensor::<DummyAdc>::resistance_to_temp_x10(100_000).unwrap();
        assert_eq!(temp, 250);

        // 40K ohms ≈ 45°C
        let temp = Ntc100kSensor::<DummyAdc>::resistance_to_temp_x10(40_000).unwrap();
        assert!((temp - 450).abs() < 50); // Within 5°C

        // 25K ohms ≈ 55°C
        let temp = Ntc100kSensor::<DummyAdc>::resistance_to_temp_x10(25_000).unwrap();
        assert!((temp - 550).abs() < 50);
    }

    #[test]
    fn test_adc_to_resistance() {
        // With 4.7K pullup and 12-bit ADC:
        // At 100K NTC: adc = 4096 * 100K / (4.7K + 100K) ≈ 3913
        let sensor = Ntc100kSensor::new(DummyAdc(0), 3300, 4700);

        // Mid-range ADC should give reasonable resistance
        let r = sensor.adc_to_resistance(2048).unwrap();
        assert!(r > 0);
        assert!(r < 1_000_000);
    }

    #[test]
    fn test_open_circuit() {
        let sensor = Ntc100kSensor::new(DummyAdc(0), 3300, 4700);

        // ADC at max = open circuit
        let result = sensor.adc_to_resistance(4095);
        assert!(matches!(result, Err(SensorError::OpenCircuit)));
    }

    #[test]
    fn test_short_circuit() {
        let sensor = Ntc100kSensor::new(DummyAdc(0), 3300, 4700);

        // ADC at zero = short circuit
        let result = sensor.adc_to_resistance(0);
        assert!(matches!(result, Err(SensorError::ShortCircuit)));
    }
}
