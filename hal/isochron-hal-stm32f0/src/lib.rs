//! STM32F0-specific HAL for the Isochron firmware
//!
//! This crate provides STM32F0-specific implementations and utilities
//! for use with `isochron-hal` traits. It supports various STM32F0 chips including:
//!
//! - STM32F042F6 (used in V0 display)
//!
//! # Features
//!
//! - `stm32f042f6` - Enable support for STM32F042F6P6 (V0 display MCU)
//! - `defmt` - Enable debug formatting support
//!
//! # Usage
//!
//! This HAL provides constants, configuration types, and error converters
//! for STM32F0 peripherals. The display firmware uses these along with
//! embassy-stm32 directly for peripheral access.

#![no_std]

pub mod flash;
pub mod gpio;
pub mod i2c;
pub mod uart;

// Re-export shared types from isochron-hal
pub use isochron_hal::flash::StorageKey;
