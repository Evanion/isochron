//! Calibration data persistence
//!
//! Loads and saves PID calibration data to flash storage.

use defmt::*;

use isochron_core::config::{CalibrationData, HeaterCalibration};
use isochron_hal_rp2040::flash::{FlashError, FlashStorage, StorageKey};
use isochron_hal_rp2040::FlashStorageTrait;

/// Maximum serialized calibration size
const MAX_CALIBRATION_SIZE: usize = 256;

/// Calibration persistence errors
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CalibrationError {
    /// Flash operation failed
    Flash(FlashError),
    /// Deserialization failed
    Deserialize,
    /// Serialization failed
    Serialize,
    /// CRC check failed
    CrcMismatch,
    /// Invalid magic or version
    InvalidFormat,
}

impl From<FlashError> for CalibrationError {
    fn from(e: FlashError) -> Self {
        CalibrationError::Flash(e)
    }
}

/// Load calibration data from flash
///
/// Returns the stored calibration data, or a new empty CalibrationData
/// if no calibration is stored or the data is invalid.
pub async fn load_calibration(storage: &mut FlashStorage<'_>) -> CalibrationData {
    match load_calibration_inner(storage).await {
        Ok(data) => {
            info!("Loaded PID calibration from flash");
            log_calibration_summary(&data);
            data
        }
        Err(CalibrationError::Flash(FlashError::NotFound)) => {
            debug!("No calibration data in flash, using defaults");
            CalibrationData::new()
        }
        Err(e) => {
            warn!("Failed to load calibration: {:?}, using defaults", e);
            CalibrationData::new()
        }
    }
}

/// Inner function that returns errors
async fn load_calibration_inner(
    storage: &mut FlashStorage<'_>,
) -> Result<CalibrationData, CalibrationError> {
    let mut buffer = [0u8; MAX_CALIBRATION_SIZE];
    let len = storage
        .read(StorageKey::PidCalibration, &mut buffer)
        .await?;

    debug!("Read {} bytes of calibration from flash", len);

    // Deserialize with postcard
    let data: CalibrationData =
        postcard::from_bytes(&buffer[..len]).map_err(|_| CalibrationError::Deserialize)?;

    // Validate magic and version
    if !data.is_valid() {
        return Err(CalibrationError::InvalidFormat);
    }

    // Verify CRC
    if !data.verify_crc() {
        warn!("Calibration CRC mismatch");
        return Err(CalibrationError::CrcMismatch);
    }

    Ok(data)
}

/// Save calibration data to flash
///
/// Updates the CRC before saving.
pub async fn save_calibration(
    storage: &mut FlashStorage<'_>,
    data: &mut CalibrationData,
) -> Result<(), CalibrationError> {
    // Update CRC before saving
    data.update_crc();

    // Serialize with postcard
    let mut buffer = [0u8; MAX_CALIBRATION_SIZE];
    let bytes = postcard::to_slice(data, &mut buffer).map_err(|_| CalibrationError::Serialize)?;

    debug!("Saving {} bytes of calibration to flash", bytes.len());

    storage
        .write(StorageKey::PidCalibration, bytes)
        .await
        .map_err(CalibrationError::Flash)?;

    info!("Saved PID calibration to flash");
    log_calibration_summary(data);

    Ok(())
}

/// Save a single heater calibration
///
/// Loads existing calibration, updates the heater entry, and saves.
pub async fn save_heater_calibration(
    storage: &mut FlashStorage<'_>,
    calibration: HeaterCalibration,
) -> Result<(), CalibrationError> {
    // Load existing data (or create new)
    let mut data = load_calibration(storage).await;

    // Update the heater entry
    if !data.set(calibration) {
        warn!("No available slot for heater calibration");
        // Still save what we have
    }

    save_calibration(storage, &mut data).await
}

/// Clear calibration for a specific heater
#[allow(dead_code)]
pub async fn clear_heater_calibration(
    storage: &mut FlashStorage<'_>,
    heater_index: u8,
) -> Result<(), CalibrationError> {
    let mut data = load_calibration(storage).await;
    data.clear_heater(heater_index);
    save_calibration(storage, &mut data).await
}

/// Log a summary of calibration data
fn log_calibration_summary(data: &CalibrationData) {
    let valid_count = data.heaters.iter().filter(|h| h.is_valid()).count();
    debug!("Calibration: {} heater(s) configured", valid_count);

    for heater in &data.heaters {
        if heater.is_valid() {
            debug!(
                "  Heater {}: Kp={}.{:02}, Ki={}.{:02}, Kd={}.{:02}",
                heater.heater_index,
                heater.kp_x100 / 100,
                (heater.kp_x100 % 100).abs(),
                heater.ki_x100 / 100,
                (heater.ki_x100 % 100).abs(),
                heater.kd_x100 / 100,
                (heater.kd_x100 % 100).abs(),
            );
        }
    }
}
