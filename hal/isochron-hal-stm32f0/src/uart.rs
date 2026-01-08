//! UART driver for STM32F0
//!
//! Provides UART communication for the isochron protocol.

use embassy_stm32::usart::Error as UsartError;

/// UART configuration
#[derive(Debug, Clone, Copy)]
pub struct UartConfig {
    /// Baud rate
    pub baudrate: u32,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baudrate: 250_000, // Default for isochron protocol
        }
    }
}

/// Error from UART operations
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum UartBusError {
    /// Framing error
    Framing,
    /// Noise error
    Noise,
    /// Overrun error
    Overrun,
    /// Parity error
    Parity,
    /// Buffer too small
    BufferTooSmall,
    /// Other error
    Other,
}

impl From<UsartError> for UartBusError {
    fn from(e: UsartError) -> Self {
        match e {
            UsartError::Framing => UartBusError::Framing,
            UsartError::Noise => UartBusError::Noise,
            UsartError::Overrun => UartBusError::Overrun,
            UsartError::Parity => UartBusError::Parity,
            UsartError::BufferTooLong => UartBusError::BufferTooSmall,
            _ => UartBusError::Other,
        }
    }
}
