//! X-axis position stepper control task
//!
//! Controls the X-axis motor for automated jar positioning.
//! Receives position commands and homing commands, reports status.

use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_rp::peripherals::PIO1;
use embassy_time::{Duration, Instant, Ticker};

use isochron_core::motion::{Axis, HomingCommand, PositionError, PositionStatus};
use isochron_hal_rp2040::position_stepper::{PositionError as HalPositionError, PositionStepper};

use crate::channels::{HOMING_CMD, POSITION_STATUS, X_POSITION_CMD};

/// Update interval for position tracking (10ms)
#[allow(dead_code)] // Will be used when position tracking is implemented
const UPDATE_INTERVAL_MS: u64 = 10;

/// X-axis position stepper task
///
/// Controls the X-axis motor for moving to jar positions.
/// Uses PIO1 state machine 1 for the X motor.
#[embassy_executor::task]
pub async fn x_stepper_task(mut stepper: PositionStepper<'static, PIO1, 1>) {
    info!("X stepper task started");

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
                if matches!(cmd, HomingCommand::HomeX) {
                    info!("X homing started");
                    if let Err(e) = stepper.start_homing() {
                        error!("X homing failed to start: {:?}", e);
                        let status = PositionStatus::error(Axis::X, hal_error_to_core(e));
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
        if let Some(target_mm) = X_POSITION_CMD.try_take() {
            info!("X move to {} mm", target_mm);
            if let Err(e) = stepper.move_to(target_mm) {
                error!("X move failed to start: {:?}", e);
                let status = PositionStatus::error(Axis::X, hal_error_to_core(e));
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
                        info!("X operation complete at {} mm", stepper.position_mm());
                        // Determine if this was homing or a move
                        let status = if stepper.position_mm()
                            == stepper.config().position_endstop_mm
                                + stepper.config().homing_retract_mm as i32
                            || stepper.position_mm()
                                == stepper.config().position_endstop_mm
                                    - stepper.config().homing_retract_mm as i32
                        {
                            PositionStatus::homed(Axis::X)
                        } else {
                            PositionStatus::complete(Axis::X)
                        };
                        POSITION_STATUS.send(status).await;
                    }
                }
                Ok(false) => {
                    // Still moving, continue
                    trace!("X position: {} mm", stepper.position_mm());
                }
                Err(e) => {
                    error!("X position error: {:?}", e);
                    let status = PositionStatus::error(Axis::X, hal_error_to_core(e));
                    POSITION_STATUS.send(status).await;
                }
            }
        }
    }
}

/// Convert HAL position error to core position error
#[allow(dead_code)] // Will be used when position error handling is implemented
fn hal_error_to_core(e: HalPositionError) -> PositionError {
    match e {
        HalPositionError::OutOfBounds => PositionError::OutOfBounds,
        HalPositionError::NotHomed => PositionError::NotHomed,
        HalPositionError::HomingFailed => PositionError::EndstopNotTriggered,
        HalPositionError::Timeout => PositionError::Timeout,
    }
}
