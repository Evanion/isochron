//! V0 Display Communication Protocol
//!
//! This crate defines the UART-based protocol between the SKR Pico (main controller)
//! and the V0 Display (UI terminal). The protocol is designed for simplicity,
//! low latency, and robustness.
//!
//! # Protocol Overview
//!
//! All messages use a simple binary frame format:
//! ```text
//! ┌───────┬────────┬──────┬─────────────┬──────────┐
//! │ START │ LENGTH │ TYPE │ PAYLOAD     │ CHECKSUM │
//! │ 1B    │ 1B     │ 1B   │ 0–250B      │ 1B       │
//! └───────┴────────┴──────┴─────────────┴──────────┘
//! ```
//!
//! The display acts as a "dumb terminal" — it handles only input capture and
//! screen rendering. All UI logic remains on the SKR Pico.

#![no_std]
#![deny(unsafe_code)]

pub mod frame;
pub mod messages;
pub mod events;

pub use frame::{Frame, FrameError, FrameParser, FRAME_START, MAX_PAYLOAD_SIZE};
pub use messages::{ControllerCommand, DisplayCommand, PicoMessage};
pub use events::InputEvent;
