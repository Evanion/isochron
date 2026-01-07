//! Stepper motor control task
//!
//! Receives motor commands from the controller and drives the PIO stepper.

use defmt::*;
use embassy_rp::peripherals::PIO0;

use isochron_core::traits::Direction;
use isochron_hal_rp2040::stepper::PioStepper;

use crate::channels::MOTOR_CMD;

/// Stepper control task for the spin motor
///
/// Waits for motor commands and controls the PIO stepper accordingly.
/// Uses PIO0 state machine 0 for the primary spin motor.
#[embassy_executor::task]
pub async fn stepper_task(mut stepper: PioStepper<'static, PIO0, 0>) {
    info!("Stepper task started");

    // Start disabled
    stepper.disable();

    // Track last command for change detection
    let mut last_rpm: u16 = 0;
    let mut last_direction = Direction::Clockwise;

    loop {
        // Wait for next motor command
        let cmd = MOTOR_CMD.wait().await;

        trace!("Motor command: rpm={}, dir={:?}", cmd.rpm, cmd.direction);

        // Handle direction change (must stop first if changing direction)
        if cmd.direction != last_direction && last_rpm > 0 && cmd.rpm > 0 {
            // Direction change while running - stop first
            debug!("Direction change: stopping for direction reversal");
            stepper.stop();
            stepper.set_direction(cmd.direction == Direction::Clockwise);
            last_direction = cmd.direction;
        } else if cmd.direction != last_direction {
            // Can change direction while stopped
            stepper.set_direction(cmd.direction == Direction::Clockwise);
            last_direction = cmd.direction;
        }

        // Handle speed change
        if cmd.rpm != last_rpm {
            if cmd.rpm == 0 {
                // Stop motor
                debug!("Motor stop");
                stepper.stop();
                stepper.disable();
            } else {
                // Start or change speed
                if last_rpm == 0 {
                    // Starting from stopped
                    debug!("Motor start: {} RPM", cmd.rpm);
                    stepper.enable();
                } else {
                    debug!("Motor speed change: {} -> {} RPM", last_rpm, cmd.rpm);
                }
                stepper.set_rpm(cmd.rpm);
            }
            last_rpm = cmd.rpm;
        }
    }
}
