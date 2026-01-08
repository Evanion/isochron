//! I2C bus abstractions
//!
//! Provides traits for I2C master operations that can be implemented
//! by chip-specific HALs.

/// I2C bus master
///
/// Provides basic I2C read/write operations for communicating with
/// peripheral devices.
pub trait I2cBus {
    /// Error type for I2C operations
    type Error;

    /// Write data to a device at the given address
    ///
    /// # Arguments
    /// * `address` - 7-bit I2C address
    /// * `data` - Bytes to write
    fn write(&mut self, address: u8, data: &[u8]) -> Result<(), Self::Error>;

    /// Read data from a device at the given address
    ///
    /// # Arguments
    /// * `address` - 7-bit I2C address
    /// * `buf` - Buffer to read into
    fn read(&mut self, address: u8, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Write then read in a single transaction (repeated start)
    ///
    /// This is commonly used to write a register address then read data.
    ///
    /// # Arguments
    /// * `address` - 7-bit I2C address
    /// * `write_data` - Bytes to write (typically register address)
    /// * `read_buf` - Buffer to read into
    fn write_read(
        &mut self,
        address: u8,
        write_data: &[u8],
        read_buf: &mut [u8],
    ) -> Result<(), Self::Error>;
}

/// I2C configuration
#[derive(Debug, Clone, Copy)]
pub struct I2cConfig {
    /// Clock frequency in Hz
    pub frequency: u32,
}

impl Default for I2cConfig {
    fn default() -> Self {
        Self {
            frequency: 100_000, // 100kHz standard mode
        }
    }
}

impl I2cConfig {
    /// Standard mode (100 kHz)
    pub const STANDARD: Self = Self { frequency: 100_000 };

    /// Fast mode (400 kHz)
    pub const FAST: Self = Self { frequency: 400_000 };

    /// Fast mode plus (1 MHz)
    pub const FAST_PLUS: Self = Self {
        frequency: 1_000_000,
    };
}
