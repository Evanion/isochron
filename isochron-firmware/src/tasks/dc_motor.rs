//! DC motor control task
//!
//! Receives motor commands from the controller and drives DC motors via PWM.
//! Uses the isochron_drivers::motor::DcMotor driver for soft start/stop.

use defmt::*;
use embassy_rp::gpio::Output;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Ticker};

use isochron_core::traits::Direction;
use isochron_core::traits::{DcMotorDriver, MotorDriver};
use isochron_drivers::motor::dc::{DcMotor, DcMotorConfig};

use crate::channels::MOTOR_CMD;

/// DC motor configuration for the firmware
pub struct DcMotorFwConfig {
    /// Minimum duty cycle percentage
    pub min_duty: u8,
    /// Soft start ramp time in ms
    pub soft_start_ms: u16,
    /// Soft stop ramp time in ms
    pub soft_stop_ms: u16,
    /// PWM top value (determines frequency)
    pub pwm_top: u16,
}

impl Default for DcMotorFwConfig {
    fn default() -> Self {
        Self {
            min_duty: 20,
            soft_start_ms: 500,
            soft_stop_ms: 300,
            pwm_top: 1000, // 125kHz / 1000 = 125Hz base, further divided
        }
    }
}

/// DC motor control task for the basket motor
///
/// Waits for motor commands and controls the DC motor via PWM.
/// The motor command's RPM field is interpreted as speed percentage (0-100).
#[embassy_executor::task]
pub async fn dc_motor_task(
    mut pwm: Pwm<'static>,
    mut dir_pin: Option<Output<'static>>,
    mut enable_pin: Option<Output<'static>>,
    config: DcMotorFwConfig,
) {
    info!("DC motor task started");

    // Create the motor driver
    let driver_config = DcMotorConfig {
        min_duty: config.min_duty,
        soft_start_ms: config.soft_start_ms,
        soft_stop_ms: config.soft_stop_ms,
        has_direction: dir_pin.is_some(),
    };
    let mut motor = DcMotor::new(driver_config);

    // Configure PWM
    let mut pwm_config = PwmConfig::default();
    pwm_config.top = config.pwm_top;
    pwm_config.compare_a = 0; // Start at 0% duty
    pwm.set_config(&pwm_config);

    // Start with motor disabled
    if let Some(ref mut en) = enable_pin {
        en.set_low(); // Assuming active-high enable
    }

    // Track last command for change detection
    let mut last_speed: u8 = 0;
    let mut last_direction = Direction::Clockwise;
    let mut motor_running = false;

    // Update ticker - 1ms for smooth ramping
    let mut ticker = Ticker::every(Duration::from_millis(1));

    loop {
        // Check for new motor command (non-blocking)
        if let Some(cmd) = MOTOR_CMD.try_take() {
            // Interpret RPM as speed percentage (0-100)
            let speed = (cmd.rpm.min(100)) as u8;

            trace!(
                "DC Motor command: speed={}%, dir={:?}",
                speed,
                cmd.direction
            );

            // Handle direction change
            if cmd.direction != last_direction {
                if motor_running {
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

            // Handle speed change
            if speed != last_speed {
                if speed == 0 {
                    // Stop motor
                    debug!("DC Motor stop");
                    motor.stop();
                    motor_running = false;
                } else {
                    // Set speed and start if needed
                    motor.set_speed(speed);
                    if !motor_running || motor.is_stopped() {
                        debug!("DC Motor start: {}%", speed);
                        motor.enable(true);
                        if let Some(ref mut en) = enable_pin {
                            en.set_high();
                        }
                        if let Err(e) = motor.start() {
                            warn!("Failed to start motor: {:?}", e);
                        } else {
                            motor_running = true;
                        }
                    } else {
                        debug!("DC Motor speed change: {}% -> {}%", last_speed, speed);
                    }
                }
                last_speed = speed;
            }
        }

        // Update motor driver (handles ramping)
        let duty = motor.update();

        // Apply duty cycle to PWM
        let compare = (duty as u32 * config.pwm_top as u32 / 100) as u16;
        pwm_config.compare_a = compare;
        pwm.set_config(&pwm_config);

        // Disable when fully stopped
        if motor.is_stopped() && motor_running {
            motor_running = false;
            motor.enable(false);
            if let Some(ref mut en) = enable_pin {
                en.set_low();
            }
            debug!("DC Motor fully stopped");
        }

        ticker.next().await;
    }
}
