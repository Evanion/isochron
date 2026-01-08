//! SPI bus abstractions
//!
//! Provides traits for SPI master operations that can be implemented
//! by chip-specific HALs.

/// SPI bus master
///
/// Provides basic SPI transfer operations for communicating with
/// peripheral devices.
pub trait SpiBus {
    /// Error type for SPI operations
    type Error;

    /// Transfer data (simultaneous read/write)
    ///
    /// Writes data from `write` buffer while reading into `read` buffer.
    /// Both buffers must be the same length.
    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error>;

    /// Write data without reading
    fn write(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Read data (writes zeros)
    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Transfer data in place
    ///
    /// Writes data from buffer while reading into the same buffer.
    fn transfer_in_place(&mut self, data: &mut [u8]) -> Result<(), Self::Error>;
}

/// SPI configuration
#[derive(Debug, Clone, Copy)]
pub struct SpiConfig {
    /// Clock frequency in Hz
    pub frequency: u32,
    /// Clock polarity
    pub polarity: Polarity,
    /// Clock phase
    pub phase: Phase,
}

impl Default for SpiConfig {
    fn default() -> Self {
        Self {
            frequency: 1_000_000, // 1 MHz
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        }
    }
}

/// SPI clock polarity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// Clock idles low (CPOL=0)
    IdleLow,
    /// Clock idles high (CPOL=1)
    IdleHigh,
}

/// SPI clock phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Data captured on first clock transition (CPHA=0)
    CaptureOnFirstTransition,
    /// Data captured on second clock transition (CPHA=1)
    CaptureOnSecondTransition,
}

/// SPI mode (combined polarity and phase)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Mode 0: CPOL=0, CPHA=0
    Mode0,
    /// Mode 1: CPOL=0, CPHA=1
    Mode1,
    /// Mode 2: CPOL=1, CPHA=0
    Mode2,
    /// Mode 3: CPOL=1, CPHA=1
    Mode3,
}

impl From<Mode> for (Polarity, Phase) {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Mode0 => (Polarity::IdleLow, Phase::CaptureOnFirstTransition),
            Mode::Mode1 => (Polarity::IdleLow, Phase::CaptureOnSecondTransition),
            Mode::Mode2 => (Polarity::IdleHigh, Phase::CaptureOnFirstTransition),
            Mode::Mode3 => (Polarity::IdleHigh, Phase::CaptureOnSecondTransition),
        }
    }
}
