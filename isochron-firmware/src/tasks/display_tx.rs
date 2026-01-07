//! Display UART transmit task
//!
//! Sends screen updates and heartbeat responses to the V0 Display.

use defmt::*;
use embassy_rp::uart::BufferedUartTx;
use embassy_rp::peripherals::UART0;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use embedded_io_async::Write;

use crate::channels::{HEARTBEAT_RECEIVED, SCREEN_UPDATE};
use crate::display::{protocol, Screen};

/// Shared screen buffer protected by mutex
pub static SCREEN_BUFFER: Mutex<CriticalSectionRawMutex, Screen> = Mutex::new(Screen::new());

/// Display TX task - sends frames to V0 Display
#[embassy_executor::task]
pub async fn display_tx_task(mut tx: BufferedUartTx<'static, UART0>) {
    info!("Display TX task started");

    // Ticker for checking heartbeat response
    let mut ticker = Ticker::every(Duration::from_millis(50));

    loop {
        // Check for pending heartbeat response
        if HEARTBEAT_RECEIVED.signaled() {
            HEARTBEAT_RECEIVED.reset();
            send_pong(&mut tx).await;
        }

        // Check for screen update request
        if SCREEN_UPDATE.signaled() {
            SCREEN_UPDATE.reset();
            send_screen_update(&mut tx).await;
        }

        ticker.next().await;
    }
}

/// Send PONG response to display
async fn send_pong(tx: &mut BufferedUartTx<'static, UART0>) {
    if let Ok(frame) = protocol::pong_frame() {
        let mut buf = [0u8; 64];
        if let Ok(len) = frame.encode(&mut buf) {
            if let Err(e) = tx.write_all(&buf[..len]).await {
                warn!("Failed to send PONG: {:?}", e);
            } else {
                trace!("PONG sent");
            }
        }
    }
}

/// Send current screen content to display
async fn send_screen_update(tx: &mut BufferedUartTx<'static, UART0>) {
    // Lock screen buffer and encode frames
    let screen = SCREEN_BUFFER.lock().await;

    for frame in protocol::encode_screen(&screen) {
        let mut buf = [0u8; 64];
        if let Ok(len) = frame.encode(&mut buf) {
            if let Err(e) = tx.write_all(&buf[..len]).await {
                warn!("Failed to send screen frame: {:?}", e);
                break;
            }
        }
    }

    trace!("Screen update sent");
}
