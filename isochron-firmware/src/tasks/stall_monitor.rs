//! Motor stall monitoring task
//!
//! Monitors the TMC2209 DIAG pin for StallGuard stall detection.
//! When a stall is detected (DIAG goes high), signals the controller.

use defmt::*;
use embassy_rp::gpio::Input;
use embassy_time::{Duration, Ticker};

use crate::channels::MOTOR_STALL;

/// Stall monitor configuration
pub struct StallMonitorConfig {
    /// Debounce time in milliseconds
    pub debounce_ms: u32,
    /// Active level (true = high when stalled)
    pub active_high: bool,
}

impl Default for StallMonitorConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 50,
            active_high: true, // TMC2209 DIAG is active high
        }
    }
}

/// Stall monitor task
///
/// Monitors the TMC2209 DIAG pin and signals stall conditions to the controller.
/// Uses debouncing to prevent spurious stall detection.
#[embassy_executor::task]
pub async fn stall_monitor_task(diag_pin: Input<'static>, config: StallMonitorConfig) {
    info!("Stall monitor task started");

    let mut ticker = Ticker::every(Duration::from_millis(20));
    let mut stalled = false;
    let mut debounce_counter: u32 = 0;
    let debounce_threshold = config.debounce_ms / 20; // 20ms tick rate

    loop {
        let pin_stalled = if config.active_high {
            diag_pin.is_high()
        } else {
            diag_pin.is_low()
        };

        if pin_stalled {
            debounce_counter = debounce_counter.saturating_add(1);
            if debounce_counter >= debounce_threshold && !stalled {
                stalled = true;
                warn!("Motor stall detected!");
                MOTOR_STALL.signal(true);
            }
        } else {
            if stalled && debounce_counter == 0 {
                stalled = false;
                info!("Motor stall cleared");
                MOTOR_STALL.signal(false);
            }
            debounce_counter = debounce_counter.saturating_sub(1);
        }

        ticker.next().await;
    }
}
