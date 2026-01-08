//! Display UART receive task
//!
//! Receives frames from the V0 Display and dispatches events.

use defmt::*;
use embassy_rp::uart::BufferedUartRx;
use embedded_io_async::Read;

use isochron_protocol::{DisplayCommand, FrameParser};

use crate::channels::{HEARTBEAT_RECEIVED, INPUT_CHANNEL};

/// Buffer size for UART receive
const RX_BUF_SIZE: usize = 64;

/// Display RX task - receives and parses frames from V0 Display
#[embassy_executor::task]
pub async fn display_rx_task(mut rx: BufferedUartRx) {
    info!("Display RX task started");

    let mut parser = FrameParser::new();
    let mut buf = [0u8; RX_BUF_SIZE];

    loop {
        // Read available bytes
        match rx.read(&mut buf).await {
            Ok(n) if n > 0 => {
                trace!("RX: {} bytes", n);

                // Feed bytes to parser
                for &byte in &buf[..n] {
                    match parser.feed(byte) {
                        Ok(Some(frame)) => {
                            // Parse the display command
                            match DisplayCommand::from_frame(&frame) {
                                Ok(cmd) => {
                                    handle_display_command(cmd).await;
                                }
                                Err(e) => {
                                    warn!("Failed to parse display command: {:?}", e);
                                }
                            }
                        }
                        Ok(None) => {
                            // Need more bytes
                        }
                        Err(e) => {
                            warn!("Frame parse error: {:?}", e);
                        }
                    }
                }
            }
            Ok(_) => {
                // No bytes read, continue
            }
            Err(e) => {
                warn!("UART read error: {:?}", e);
            }
        }
    }
}

/// Handle a parsed display command
async fn handle_display_command(cmd: DisplayCommand) {
    match cmd {
        DisplayCommand::Ping => {
            trace!("PING received");
            HEARTBEAT_RECEIVED.signal(());
        }
        DisplayCommand::Input(event) => {
            debug!("Input event: {:?}", event);
            // Send to input channel, dropping if full
            if INPUT_CHANNEL.try_send(event).is_err() {
                warn!("Input channel full, dropping event");
            }
        }
        DisplayCommand::Ack { seq: _ } => {
            // ACK received, could use for flow control
            trace!("ACK received");
        }
    }
}
