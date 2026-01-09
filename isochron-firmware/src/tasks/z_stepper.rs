//! Z-axis position stepper control task
//!
//! Controls the Z-axis motor for automated lift/lower operations.
//! Receives position commands and homing commands, reports status.

use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_rp::peripherals::PIO1;
use embassy_time::{Duration, Instant, Ticker};

use isochron_core::motion::{Axis, HomingCommand, PositionError, PositionStatus};
use isochron_hal_rp2040::position_stepper::{PositionError as HalPositionError, PositionStepper};

use crate::channels::{HOMING_CMD, POSITION_STATUS, Z_POSITION_CMD};

/// Update interval for position tracking (10ms)
const UPDATE_INTERVAL_MS: u64 = 10;

/// Z-axis position stepper task
///
/// Controls the Z-axis motor for lifting and lowering the basket.
/// Uses PIO1 state machine 0 for the Z motor.
#[embassy_executor::task]
pub async fn z_stepper_task(mut stepper: PositionStepper<'static, PIO1, 0>) {
    info!("Z stepper task started");

    // Start disabled
    stepper.disable();

    let mut ticker = Ticker::every(Duration::from_millis(UPDATE_INTERVAL_MS));
    let mut last_update = Instant::now();

    loop {
        // Wait for either a command or the update tick
        let result = select(
            async {
                // Check for homing command first (higher priority)
                let cmd = HOMING_CMD.wait().await;
                Some(cmd)
            },
            ticker.next(),
        )
        .await;

        match result {
            Either::First(Some(cmd)) => {
                // Homing command received
                if matches!(cmd, HomingCommand::HomeZ) {
                    info!("Z homing started");
                    if let Err(e) = stepper.start_homing() {
                        error!("Z homing failed to start: {:?}", e);
                        let status = PositionStatus::error(Axis::Z, hal_error_to_core(e));
                        POSITION_STATUS.send(status).await;
                    }
                }
            }
            Either::First(None) => {
                // Signal was reset without value, continue
            }
            Either::Second(_) => {
                // Ticker fired - update position and check for move command
            }
        }

        // Check for position command (non-blocking)
        if let Some(target_mm) = Z_POSITION_CMD.try_take() {
            info!("Z move to {} mm", target_mm);
            if let Err(e) = stepper.move_to(target_mm) {
                error!("Z move failed to start: {:?}", e);
                let status = PositionStatus::error(Axis::Z, hal_error_to_core(e));
                POSITION_STATUS.send(status).await;
            }
        }

        // Update position tracking
        let now = Instant::now();
        let delta_ms = (now - last_update).as_millis() as u32;
        last_update = now;

        if stepper.is_moving() {
            let current_ms = now.as_millis();
            match stepper.update(delta_ms, current_ms) {
                Ok(true) => {
                    // Operation completed
                    if stepper.is_homed() {
                        info!("Z operation complete at {} mm", stepper.position_mm());
                        // Determine if this was homing or a move
                        // If we just finished homing, send Homed status
                        // Otherwise send Complete status
                        // For simplicity, check if position is near endstop
                        let status = if stepper.position_mm()
                            == stepper.config().position_endstop_mm
                                + stepper.config().homing_retract_mm as i32
                            || stepper.position_mm()
                                == stepper.config().position_endstop_mm
                                    - stepper.config().homing_retract_mm as i32
                        {
                            PositionStatus::homed(Axis::Z)
                        } else {
                            PositionStatus::complete(Axis::Z)
                        };
                        POSITION_STATUS.send(status).await;
                    }
                }
                Ok(false) => {
                    // Still moving, continue
                    trace!("Z position: {} mm", stepper.position_mm());
                }
                Err(e) => {
                    error!("Z position error: {:?}", e);
                    let status = PositionStatus::error(Axis::Z, hal_error_to_core(e));
                    POSITION_STATUS.send(status).await;
                }
            }
        }
    }
}

/// Convert HAL position error to core position error
fn hal_error_to_core(e: HalPositionError) -> PositionError {
    match e {
        HalPositionError::OutOfBounds => PositionError::OutOfBounds,
        HalPositionError::NotHomed => PositionError::NotHomed,
        HalPositionError::HomingFailed => PositionError::EndstopNotTriggered,
        HalPositionError::Timeout => PositionError::Timeout,
    }
}
