//! TMC2209 driver task
//!
//! Initializes and monitors TMC2209 stepper drivers via UART.
//! Uses UART1 on GPIO8 (TX) and GPIO9 (RX) for TMC communication.

use defmt::*;
use embassy_rp::peripherals::UART1;
use embassy_rp::uart::{Async, UartTx};
use embassy_time::{Duration, Timer};

use isochron_drivers::stepper::tmc2209::{Tmc2209Config, Tmc2209Driver};

/// TMC2209 initialization task
///
/// Initializes the TMC2209 driver over UART with the specified configuration.
/// After initialization, the driver is configured for StealthChop operation
/// with the specified current settings.
#[embassy_executor::task]
pub async fn tmc_init_task(mut tx: UartTx<'static, UART1, Async>, config: Tmc2209Config) {
    info!("TMC2209 init task starting...");

    // Wait for TMC2209 to power up
    Timer::after(Duration::from_millis(100)).await;

    // Create driver and get initialization datagrams
    let driver = Tmc2209Driver::new(config.clone());
    let datagrams = driver.init_datagrams();

    info!(
        "Initializing TMC2209 at address {} with {}mA run current",
        config.uart_address, config.run_current_ma
    );

    // Send each initialization datagram
    for (i, datagram) in datagrams.iter().enumerate() {
        // Small delay between writes for TMC to process
        Timer::after(Duration::from_millis(10)).await;

        match tx.write(datagram).await {
            Ok(()) => {
                trace!("Sent TMC datagram {}/6", i + 1);
            }
            Err(e) => {
                error!("Failed to send TMC datagram {}: {:?}", i + 1, e);
                return;
            }
        }
    }

    // Small delay to ensure final bytes are sent
    Timer::after(Duration::from_millis(10)).await;

    info!("TMC2209 initialized successfully");
    debug!("  Microsteps: {}", config.microsteps);
    debug!("  Run current: {}mA", config.run_current_ma);
    debug!("  Hold current: {}mA", config.hold_current_ma);
    debug!("  StealthChop: {}", config.stealthchop);

    // Task complete - TMC2209 is now configured
    // The stepper task handles step/dir/enable via GPIO
    // Future: could add periodic stall monitoring here
}
