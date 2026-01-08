//! Display driver trait for the V0 Display

use isochron_protocol::InputEvent;

/// Errors that can occur with display communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// Communication timeout
    Timeout,
    /// Link lost (missed heartbeats)
    LinkLost,
    /// Protocol framing error
    FrameError,
    /// Buffer overflow
    BufferOverflow,
}

/// Trait for display communication
///
/// This trait abstracts the UART communication with the V0 Display.
/// The display acts as a dumb terminal - all UI logic stays on the Pico.
pub trait DisplayDriver {
    /// Clear the entire screen
    fn clear(&mut self) -> Result<(), DisplayError>;

    /// Draw text at a position
    ///
    /// - `row`: Row number (0-7)
    /// - `col`: Column number (0-20)
    /// - `text`: ASCII text to display (max 21 chars)
    fn text(&mut self, row: u8, col: u8, text: &str) -> Result<(), DisplayError>;

    /// Invert a region (for selection highlight)
    ///
    /// - `row`: Row to invert (0-7)
    /// - `start_col`: Starting column (0-20)
    /// - `end_col`: Ending column (0-20), inclusive
    fn invert(&mut self, row: u8, start_col: u8, end_col: u8) -> Result<(), DisplayError>;

    /// Draw a horizontal line
    fn hline(&mut self, row: u8, start_col: u8, end_col: u8) -> Result<(), DisplayError>;

    /// Send heartbeat response (PONG)
    fn pong(&mut self) -> Result<(), DisplayError>;

    /// Reset the display to boot state
    fn reset(&mut self) -> Result<(), DisplayError>;

    /// Poll for incoming input events
    ///
    /// Returns `Ok(Some(event))` if an input event is available,
    /// `Ok(None)` if no event is pending.
    fn poll_input(&mut self) -> Result<Option<InputEvent>, DisplayError>;

    /// Check if the display link is healthy
    fn is_link_healthy(&self) -> bool;

    /// Get the number of missed heartbeats
    fn missed_heartbeats(&self) -> u8;
}

/// Helper trait for drawing common UI elements
pub trait DisplayExt: DisplayDriver {
    /// Draw a menu item with optional selection
    fn draw_menu_item(&mut self, row: u8, text: &str, selected: bool) -> Result<(), DisplayError> {
        // Clear the row first with spaces
        self.text(row, 0, "                     ")?;

        // Draw the item with selection indicator
        if selected {
            // Use play indicator for selected item
            let mut buf = [0u8; 22];
            buf[0] = 0x01; // Play indicator
            buf[1] = b' ';
            let text_bytes = text.as_bytes();
            let len = text_bytes.len().min(19);
            buf[2..2 + len].copy_from_slice(&text_bytes[..len]);

            // Safe because we only use ASCII
            let display_text = core::str::from_utf8(&buf[..2 + len]).unwrap_or("");
            self.text(row, 0, display_text)?;
            self.invert(row, 0, 20)?;
        } else {
            let mut buf = [0u8; 22];
            buf[0] = b' ';
            buf[1] = b' ';
            let text_bytes = text.as_bytes();
            let len = text_bytes.len().min(19);
            buf[2..2 + len].copy_from_slice(&text_bytes[..len]);

            let display_text = core::str::from_utf8(&buf[..2 + len]).unwrap_or("");
            self.text(row, 0, display_text)?;
        }

        Ok(())
    }

    /// Draw a label-value pair
    fn draw_field(&mut self, row: u8, label: &str, value: &str) -> Result<(), DisplayError> {
        // Format: "Label:     Value"
        let mut buf = [b' '; 21];

        let label_bytes = label.as_bytes();
        let label_len = label_bytes.len().min(10);
        buf[..label_len].copy_from_slice(&label_bytes[..label_len]);

        if label_len < 11 {
            buf[label_len] = b':';
        }

        let value_bytes = value.as_bytes();
        let value_len = value_bytes.len().min(9);
        let value_start = 21 - value_len;
        buf[value_start..21].copy_from_slice(&value_bytes[..value_len]);

        let display_text = core::str::from_utf8(&buf).unwrap_or("");
        self.text(row, 0, display_text)
    }
}

// Blanket implementation for all DisplayDriver types
impl<T: DisplayDriver> DisplayExt for T {}
