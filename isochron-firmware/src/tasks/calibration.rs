//! Calibration persistence task
//!
//! Handles saving and loading PID calibration data to/from flash.
//! Runs as a background task, listening for save requests.

use defmt::*;

use isochron_core::config::HeaterCalibration;
use isochron_hal_rp2040::flash::FlashStorage;

use crate::channels::CALIBRATION_SAVE;
use crate::config::calibration::save_heater_calibration;

/// Calibration task - handles flash persistence for PID calibration
///
/// This task owns the flash storage and handles save requests from
/// the controller. Saving happens asynchronously to avoid blocking
/// other tasks during flash operations.
#[embassy_executor::task]
pub async fn calibration_task(mut storage: FlashStorage<'static>) {
    info!("Calibration task started");

    loop {
        // Wait for a save request
        let request = CALIBRATION_SAVE.wait().await;

        info!(
            "Saving calibration for heater {}: Kp={}.{:02}, Ki={}.{:02}, Kd={}.{:02}",
            request.heater_index,
            request.kp_x100 / 100,
            (request.kp_x100 % 100).abs(),
            request.ki_x100 / 100,
            (request.ki_x100 % 100).abs(),
            request.kd_x100 / 100,
            (request.kd_x100 % 100).abs(),
        );

        // Create calibration entry
        let calibration = HeaterCalibration::new(
            request.heater_index,
            request.kp_x100,
            request.ki_x100,
            request.kd_x100,
        );

        // Save to flash
        match save_heater_calibration(&mut storage, calibration).await {
            Ok(()) => {
                info!("Calibration saved successfully");
            }
            Err(e) => {
                error!("Failed to save calibration: {:?}", e);
            }
        }
    }
}
