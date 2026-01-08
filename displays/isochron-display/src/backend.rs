//! Display backend trait
//!
//! Defines the interface for different display types.

/// Display backend errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// Communication error with display
    Communication,
    /// Invalid coordinates or dimensions
    InvalidCoordinates,
    /// Display not initialized
    NotInitialized,
    /// Buffer overflow
    BufferOverflow,
}

/// Display backend trait
///
/// Provides a hardware-agnostic interface for rendering to displays.
/// Implementations handle the specifics of OLED, TFT, or other display types.
pub trait DisplayBackend {
    /// Clear the entire display
    fn clear(&mut self) -> Result<(), DisplayError>;

    /// Draw text at the specified row and column
    ///
    /// - `row`: Row number (0-based)
    /// - `col`: Column number in characters (0-based)
    /// - `text`: Text to display
    fn draw_text(&mut self, row: u8, col: u8, text: &str) -> Result<(), DisplayError>;

    /// Invert a region on the specified row (for selection highlighting)
    ///
    /// - `row`: Row number
    /// - `start_col`: Starting column
    /// - `end_col`: Ending column (exclusive)
    fn invert_region(&mut self, row: u8, start_col: u8, end_col: u8) -> Result<(), DisplayError>;

    /// Flush buffered content to the display
    ///
    /// For displays with internal buffers, this sends the buffer to the hardware.
    fn flush(&mut self) -> Result<(), DisplayError>;

    /// Get the display dimensions
    ///
    /// Returns (columns, rows) in character units
    fn dimensions(&self) -> (u8, u8);

    /// Check if the display is ready
    fn is_ready(&self) -> bool;
}

/// Extended display backend for displays supporting graphics
pub trait GraphicsDisplayBackend: DisplayBackend {
    /// Draw a horizontal line
    fn draw_hline(&mut self, x: u16, y: u16, length: u16) -> Result<(), DisplayError>;

    /// Draw a vertical line
    fn draw_vline(&mut self, x: u16, y: u16, length: u16) -> Result<(), DisplayError>;

    /// Draw a rectangle outline
    fn draw_rect(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), DisplayError>;

    /// Fill a rectangle
    fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), DisplayError>;

    /// Get pixel dimensions
    fn pixel_dimensions(&self) -> (u16, u16);
}
