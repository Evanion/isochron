//! AC motor control task
//!
//! Receives motor commands from the controller and drives AC motors via relay.
//! Uses the isochron_drivers::motor::AcMotor driver for safe relay timing.

use defmt::*;
use embassy_rp::gpio::Output;
use embassy_time::{Duration, Ticker};

use isochron_core::traits::Direction;
use isochron_core::traits::{AcMotorDriver, MotorDriver};
use isochron_drivers::motor::ac::{AcMotor, AcMotorConfig, AcRelayType};

use crate::channels::MOTOR_CMD;

/// AC motor configuration for the firmware
pub struct AcMotorFwConfig {
    /// Relay type (mechanical or SSR)
    pub relay_type: AcRelayType,
    /// Minimum delay between relay switches (ms)
    pub min_switch_delay_ms: u32,
    /// Relay is active-high
    pub active_high: bool,
    /// Has direction control
    pub has_direction: bool,
}

impl Default for AcMotorFwConfig {
    fn default() -> Self {
        Self {
            relay_type: AcRelayType::Mechanical,
            min_switch_delay_ms: 100,
            active_high: true,
            has_direction: false,
        }
    }
}

/// AC motor control task for the basket motor
///
/// Waits for motor commands and controls the AC motor via relay.
/// The motor command's RPM field is interpreted as on (>0) or off (0).
/// AC motors have no speed control - they run at fixed speed.
#[embassy_executor::task]
pub async fn ac_motor_task(
    mut relay_pin: Output<'static>,
    mut dir_pin: Option<Output<'static>>,
    config: AcMotorFwConfig,
) {
    info!("AC motor task started");

    // Create the motor driver
    let driver_config = AcMotorConfig {
        relay_type: config.relay_type,
        min_switch_delay_ms: config.min_switch_delay_ms,
        active_high: config.active_high,
        has_direction: config.has_direction,
    };
    let mut motor = AcMotor::new(driver_config);

    // Start with motor off
    set_relay(&mut relay_pin, false, config.active_high);

    // Track last command for change detection
    let mut last_on = false;
    let mut last_direction = Direction::Clockwise;

    // Update ticker - 1ms for timing management
    let mut ticker = Ticker::every(Duration::from_millis(1));

    loop {
        // Check for new motor command (non-blocking)
        if let Some(cmd) = MOTOR_CMD.try_take() {
            // Any non-zero RPM means "on" for AC motors
            let should_be_on = cmd.rpm > 0;

            trace!(
                "AC Motor command: on={}, dir={:?}",
                should_be_on,
                cmd.direction
            );

            // Handle direction change
            if cmd.direction != last_direction && config.has_direction {
                if last_on {
                    // Must stop before changing direction
                    debug!("Direction change: stopping for reversal");
                    motor.stop();
                }
                motor.set_direction(cmd.direction);
                if let Some(ref mut dir) = dir_pin {
                    if cmd.direction == Direction::Clockwise {
                        dir.set_high();
                    } else {
                        dir.set_low();
                    }
                }
                last_direction = cmd.direction;
            }

            // Handle on/off change
            if should_be_on != last_on {
                if should_be_on {
                    // Turn on
                    debug!("AC Motor on");
                    motor.enable(true);
                    match motor.start() {
                        Ok(()) => {
                            // Relay state will be updated in the update loop
                        }
                        Err(e) => {
                            warn!("Failed to start AC motor: {:?}", e);
                        }
                    }
                } else {
                    // Turn off
                    debug!("AC Motor off");
                    motor.stop();
                }
                last_on = should_be_on;
            }
        }

        // Update motor driver (handles timing)
        motor.update();

        // Apply relay state
        let relay_state = motor.relay_state();
        set_relay(&mut relay_pin, relay_state, config.active_high);

        // Check for full stop
        if motor.is_stopped() && last_on {
            last_on = false;
            motor.enable(false);
            debug!("AC Motor fully stopped");
        }

        ticker.next().await;
    }
}

/// Set relay pin state, accounting for active-high/low configuration
fn set_relay(pin: &mut Output<'static>, on: bool, _active_high: bool) {
    // Note: The motor driver already accounts for active_high in relay_state(),
    // so we just set the pin directly
    if on {
        pin.set_high();
    } else {
        pin.set_low();
    }
}
