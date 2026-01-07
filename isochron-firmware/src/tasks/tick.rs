//! Tick task for time-based updates
//!
//! Provides periodic ticks to the controller for:
//! - Scheduler time tracking
//! - Safety monitoring (heartbeat timeout)
//! - Display refresh

use defmt::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Ticker};

/// Tick interval in milliseconds
pub const TICK_INTERVAL_MS: u32 = 100;

/// Signal to notify controller of tick
pub static TICK_SIGNAL: Signal<CriticalSectionRawMutex, u32> = Signal::new();

/// Tick task - sends periodic tick signals with timestamp
#[embassy_executor::task]
pub async fn tick_task() {
    info!("Tick task started");

    let mut ticker = Ticker::every(Duration::from_millis(TICK_INTERVAL_MS as u64));
    let start = Instant::now();

    loop {
        ticker.next().await;

        // Calculate elapsed time since start in milliseconds
        let now_ms = start.elapsed().as_millis() as u32;

        // Signal the controller
        TICK_SIGNAL.signal(now_ms);
    }
}
