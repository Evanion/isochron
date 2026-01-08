//! UART serial communication abstractions
//!
//! Provides traits for asynchronous serial communication that can be
//! implemented by chip-specific HALs.

/// UART transmitter
///
/// Async trait for sending data over a UART interface.
pub trait UartTx {
    /// Error type for transmit operations
    type Error;

    /// Write data to the UART
    ///
    /// Blocks until all data has been written or an error occurs.
    fn write_blocking(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Flush any buffered data
    fn flush(&mut self) -> Result<(), Self::Error>;
}

/// UART receiver
///
/// Async trait for receiving data from a UART interface.
pub trait UartRx {
    /// Error type for receive operations
    type Error;

    /// Read data from the UART
    ///
    /// Blocks until the buffer is filled or an error occurs.
    fn read_blocking(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

    /// Read a single byte from the UART
    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8; 1];
        self.read_blocking(&mut buf)?;
        Ok(buf[0])
    }
}

/// Combined UART interface
///
/// For UARTs that provide both TX and RX on a single peripheral.
pub trait Uart: UartTx + UartRx {}

// Blanket implementation
impl<T: UartTx + UartRx> Uart for T {}

/// UART configuration
#[derive(Debug, Clone, Copy)]
pub struct UartConfig {
    /// Baud rate in bits per second
    pub baudrate: u32,
    /// Number of data bits (typically 8)
    pub data_bits: DataBits,
    /// Parity mode
    pub parity: Parity,
    /// Number of stop bits
    pub stop_bits: StopBits,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baudrate: 115200,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }
}

/// Number of data bits per frame
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    Seven,
    Eight,
    Nine,
}

/// Parity mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// Number of stop bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    One,
    Two,
}
