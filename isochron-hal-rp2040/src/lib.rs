//! RP2040-specific HAL for the watch cleaner firmware
//!
//! This crate provides RP2040-specific implementations:
//! - GPIO allocation and management
//! - Dynamic pin allocation for config-driven setup
//! - UART peripheral allocation
//! - ADC channel management
//! - PIO-based step pulse generation
//! - Flash storage driver

#![no_std]

pub mod adc;
pub mod flash;
pub mod gpio;
pub mod pins;
pub mod pio;
pub mod stepper;
pub mod uart;
