//! I2C bus driver for STM32F0
//!
//! Provides I2C communication for peripherals like the SH1106 OLED display.

use embassy_stm32::i2c::Error as I2cError;

/// I2C configuration
#[derive(Debug, Clone, Copy)]
pub struct I2cConfig {
    /// SCL frequency in Hz
    pub frequency: u32,
}

impl Default for I2cConfig {
    fn default() -> Self {
        Self {
            frequency: 400_000, // 400 kHz (Fast mode)
        }
    }
}

// Note: embassy-stm32 v0.5 I2C has different generics (Mode, MasterMode).
// The display firmware will use embassy's I2C directly.
// This module provides configuration helpers and error types.

/// Error from I2C operations
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum I2cBusError {
    /// Bus error
    Bus,
    /// Arbitration lost
    ArbitrationLost,
    /// NACK received
    Nack,
    /// Timeout
    Timeout,
    /// CRC error
    Crc,
    /// Overrun
    Overrun,
    /// Other error
    Other,
}

impl From<I2cError> for I2cBusError {
    fn from(e: I2cError) -> Self {
        match e {
            I2cError::Bus => I2cBusError::Bus,
            I2cError::Arbitration => I2cBusError::ArbitrationLost,
            I2cError::Nack => I2cBusError::Nack,
            I2cError::Timeout => I2cBusError::Timeout,
            I2cError::Crc => I2cBusError::Crc,
            I2cError::Overrun => I2cBusError::Overrun,
            _ => I2cBusError::Other,
        }
    }
}
