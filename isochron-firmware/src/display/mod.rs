//! V0 Display communication
//!
//! Handles UART communication with the V0 Display.
//!
//! The V0 Display acts as a "dumb terminal" - it handles only input capture
//! and screen rendering. All UI logic remains on the main controller.
//!
//! # Protocol Overview
//!
//! Communication uses a simple binary frame format over UART at 115200 baud:
//! - Display → Pico: Input events (encoder rotation, button clicks), heartbeats
//! - Pico → Display: Screen commands (clear, text, invert), heartbeat responses
//!
//! The display sends periodic PING messages. If the Pico doesn't respond with
//! PONG within the timeout, the display shows a "Link Lost" error.

pub mod protocol;
pub mod renderer;

pub use renderer::{Renderer, Screen};
