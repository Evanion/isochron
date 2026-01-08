//! PID calibration data types
//!
//! Stores autotune results that can be persisted to flash and loaded on boot.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::hardware::MAX_HEATERS;

/// Magic number to identify valid calibration data
pub const CALIBRATION_MAGIC: u32 = 0x50494443; // "PIDC"

/// Current calibration data version
pub const CALIBRATION_VERSION: u8 = 1;

/// PID calibration data for a single heater
///
/// This struct is serialized to flash using postcard.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HeaterCalibration {
    /// Heater index (0-based)
    pub heater_index: u8,
    /// Whether this calibration slot is valid
    pub valid: bool,
    /// Proportional gain (value × 100)
    pub kp_x100: i16,
    /// Integral gain (value × 100)
    pub ki_x100: i16,
    /// Derivative gain (value × 100)
    pub kd_x100: i16,
    /// Ultimate gain from autotune (for reference, × 100)
    pub ku_x100: i16,
    /// Ultimate period from autotune in ticks (for reference)
    pub tu_ticks: u16,
}

impl HeaterCalibration {
    /// Create a new calibration entry
    pub const fn new(heater_index: u8, kp_x100: i16, ki_x100: i16, kd_x100: i16) -> Self {
        Self {
            heater_index,
            valid: true,
            kp_x100,
            ki_x100,
            kd_x100,
            ku_x100: 0,
            tu_ticks: 0,
        }
    }

    /// Create from autotune result values
    pub const fn from_autotune(
        heater_index: u8,
        kp_x100: i16,
        ki_x100: i16,
        kd_x100: i16,
        ku_x100: i16,
        tu_ticks: u16,
    ) -> Self {
        Self {
            heater_index,
            valid: true,
            kp_x100,
            ki_x100,
            kd_x100,
            ku_x100,
            tu_ticks,
        }
    }

    /// Check if this calibration entry is valid
    pub const fn is_valid(&self) -> bool {
        self.valid
    }

    /// Clear this calibration entry
    pub fn clear(&mut self) {
        self.valid = false;
        self.kp_x100 = 0;
        self.ki_x100 = 0;
        self.kd_x100 = 0;
        self.ku_x100 = 0;
        self.tu_ticks = 0;
    }
}

/// Complete calibration data stored in flash
///
/// Contains calibration for multiple heaters with a header
/// for data validation.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CalibrationData {
    /// Magic number for validation
    pub magic: u32,
    /// Data format version
    pub version: u8,
    /// Heater calibrations
    pub heaters: [HeaterCalibration; MAX_HEATERS],
    /// CRC32 checksum (calculated over magic..heaters)
    pub crc: u32,
}

impl Default for CalibrationData {
    fn default() -> Self {
        Self::new()
    }
}

impl CalibrationData {
    /// Create empty calibration data
    pub const fn new() -> Self {
        Self {
            magic: CALIBRATION_MAGIC,
            version: CALIBRATION_VERSION,
            heaters: [HeaterCalibration {
                heater_index: 0,
                valid: false,
                kp_x100: 0,
                ki_x100: 0,
                kd_x100: 0,
                ku_x100: 0,
                tu_ticks: 0,
            }; MAX_HEATERS],
            crc: 0,
        }
    }

    /// Check if the data is valid (magic and version match)
    pub fn is_valid(&self) -> bool {
        self.magic == CALIBRATION_MAGIC && self.version == CALIBRATION_VERSION
    }

    /// Get calibration for a specific heater index
    pub fn get(&self, heater_index: u8) -> Option<&HeaterCalibration> {
        self.heaters
            .iter()
            .find(|h| h.valid && h.heater_index == heater_index)
    }

    /// Set calibration for a heater
    ///
    /// Finds an existing slot for this heater or uses an empty slot.
    /// Returns true if successful, false if no slots available.
    pub fn set(&mut self, calibration: HeaterCalibration) -> bool {
        // First, try to find existing entry for this heater
        for slot in &mut self.heaters {
            if slot.heater_index == calibration.heater_index {
                *slot = calibration;
                return true;
            }
        }

        // Otherwise, find an empty slot
        for slot in &mut self.heaters {
            if !slot.valid {
                *slot = calibration;
                return true;
            }
        }

        false
    }

    /// Clear calibration for a specific heater
    pub fn clear_heater(&mut self, heater_index: u8) {
        for slot in &mut self.heaters {
            if slot.heater_index == heater_index {
                slot.clear();
            }
        }
    }

    /// Calculate CRC32 for the data (excluding the crc field itself)
    ///
    /// Uses a simple CRC32 implementation suitable for embedded.
    pub fn calculate_crc(&self) -> u32 {
        // Simple CRC32 calculation
        // We'll compute over the serialized bytes in practice,
        // but for validation we use a simplified approach.
        let mut crc: u32 = 0xFFFFFFFF;

        // Include magic
        crc = crc32_update(crc, &self.magic.to_le_bytes());

        // Include version
        crc = crc32_update(crc, &[self.version]);

        // Include heater data
        for heater in &self.heaters {
            crc = crc32_update(crc, &[heater.heater_index]);
            crc = crc32_update(crc, &[heater.valid as u8]);
            crc = crc32_update(crc, &heater.kp_x100.to_le_bytes());
            crc = crc32_update(crc, &heater.ki_x100.to_le_bytes());
            crc = crc32_update(crc, &heater.kd_x100.to_le_bytes());
            crc = crc32_update(crc, &heater.ku_x100.to_le_bytes());
            crc = crc32_update(crc, &heater.tu_ticks.to_le_bytes());
        }

        !crc
    }

    /// Update the CRC field
    pub fn update_crc(&mut self) {
        self.crc = self.calculate_crc();
    }

    /// Verify the CRC is correct
    pub fn verify_crc(&self) -> bool {
        self.crc == self.calculate_crc()
    }
}

/// Simple CRC32 update function (IEEE 802.3 polynomial)
fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    const POLY: u32 = 0xEDB88320;
    let mut crc = crc;

    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ POLY;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_data_default() {
        let data = CalibrationData::default();
        assert!(data.is_valid());
        assert_eq!(data.magic, CALIBRATION_MAGIC);
        assert_eq!(data.version, CALIBRATION_VERSION);
    }

    #[test]
    fn test_set_and_get_calibration() {
        let mut data = CalibrationData::new();

        let cal = HeaterCalibration::new(0, 150, 10, 50);
        assert!(data.set(cal));

        let retrieved = data.get(0);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.kp_x100, 150);
        assert_eq!(retrieved.ki_x100, 10);
        assert_eq!(retrieved.kd_x100, 50);
    }

    #[test]
    fn test_crc_consistency() {
        let mut data = CalibrationData::new();
        data.set(HeaterCalibration::new(0, 150, 10, 50));
        data.update_crc();

        assert!(data.verify_crc());

        // Modify data without updating CRC
        data.heaters[0].kp_x100 = 200;
        assert!(!data.verify_crc());
    }

    #[test]
    fn test_clear_heater() {
        let mut data = CalibrationData::new();
        data.set(HeaterCalibration::new(0, 150, 10, 50));

        assert!(data.get(0).is_some());

        data.clear_heater(0);
        assert!(data.get(0).is_none());
    }
}
