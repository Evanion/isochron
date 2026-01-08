//! RP2040-specific HAL for the watch cleaner firmware
//!
//! This crate provides RP2040-specific implementations of the shared
//! `isochron-hal` traits, plus RP2040-specific functionality:
//!
//! - GPIO allocation and management
//! - Dynamic pin allocation for config-driven setup
//! - UART peripheral allocation
//! - ADC channel management
//! - PIO-based step pulse generation
//! - Flash storage driver (implements `isochron_hal::FlashStorage`)

#![no_std]

pub mod adc;
pub mod flash;
pub mod gpio;
pub mod pins;
pub mod pio;
pub mod stepper;
pub mod uart;

// Re-export shared traits from isochron-hal for convenience
pub use isochron_hal::{FlashStorage as FlashStorageTrait, StorageKey};
