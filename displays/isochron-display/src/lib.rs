//! Display abstraction traits and shared components for Isochron
//!
//! This crate provides:
//! - `DisplayBackend` trait for different display types (OLED, TFT, etc.)
//! - `InputSource` trait for different input methods (encoder, touch, buttons)
//! - `NavigationEvent` enum for unified input handling
//! - Screen buffer types and rendering utilities
//!
//! # Architecture
//!
//! Display modules can implement these traits with their hardware-specific code.
//! The main controller firmware uses these abstractions to render UI without
//! caring about the specific display hardware.
//!
//! ## Supported Display Types
//!
//! - **External displays** (e.g., V0 display with STM32F042): The controller sends
//!   rendering commands over UART using the isochron-protocol. The display MCU
//!   receives these and renders to its local display.
//!
//! - **Direct displays** (e.g., TFT connected to RP2040): The controller directly
//!   drives the display via SPI/I2C using an implementation of `DisplayBackend`.

#![no_std]

pub mod backend;
pub mod input;
pub mod screen;

// Re-export key types
pub use backend::{DisplayBackend, DisplayError};
pub use input::{InputSource, NavigationEvent};
pub use screen::{Screen, SCREEN_COLS, SCREEN_ROWS};
