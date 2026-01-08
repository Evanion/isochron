//! Screen buffer types
//!
//! Provides a character-based screen buffer for text-mode displays.

use heapless::String;

/// Number of character rows on standard display
pub const SCREEN_ROWS: usize = 4;

/// Number of character columns on standard display
pub const SCREEN_COLS: usize = 20;

/// Maximum characters per line
pub const LINE_LEN: usize = SCREEN_COLS;

/// Screen buffer for text-mode displays
///
/// Provides a double-buffered character display that can be rendered
/// to any `DisplayBackend` implementation.
#[derive(Clone)]
pub struct Screen {
    /// Current display content
    lines: [String<LINE_LEN>; SCREEN_ROWS],
    /// Selection/highlight state per row (start_col, end_col)
    highlights: [Option<(u8, u8)>; SCREEN_ROWS],
    /// Whether the screen needs to be redrawn
    dirty: bool,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    /// Create a new empty screen
    pub fn new() -> Self {
        Self {
            lines: core::array::from_fn(|_| String::new()),
            highlights: [None; SCREEN_ROWS],
            dirty: true,
        }
    }

    /// Clear the entire screen
    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        for highlight in &mut self.highlights {
            *highlight = None;
        }
        self.dirty = true;
    }

    /// Set the content of a specific row
    pub fn set_line(&mut self, row: usize, text: &str) {
        if row < SCREEN_ROWS {
            self.lines[row].clear();
            // Truncate if too long
            let text = if text.len() > LINE_LEN {
                &text[..LINE_LEN]
            } else {
                text
            };
            let _ = self.lines[row].push_str(text);
            self.dirty = true;
        }
    }

    /// Get the content of a specific row
    pub fn get_line(&self, row: usize) -> Option<&str> {
        self.lines.get(row).map(|s| s.as_str())
    }

    /// Set highlight (invert) region for a row
    pub fn set_highlight(&mut self, row: usize, start_col: u8, end_col: u8) {
        if row < SCREEN_ROWS {
            self.highlights[row] = Some((start_col, end_col));
            self.dirty = true;
        }
    }

    /// Clear highlight for a row
    pub fn clear_highlight(&mut self, row: usize) {
        if row < SCREEN_ROWS {
            self.highlights[row] = None;
            self.dirty = true;
        }
    }

    /// Clear all highlights
    pub fn clear_all_highlights(&mut self) {
        for highlight in &mut self.highlights {
            *highlight = None;
        }
        self.dirty = true;
    }

    /// Get highlight region for a row
    pub fn get_highlight(&self, row: usize) -> Option<(u8, u8)> {
        self.highlights.get(row).copied().flatten()
    }

    /// Check if screen needs redrawing
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark screen as clean (after rendering)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Mark screen as dirty (needs redraw)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get all lines as an iterator
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|s| s.as_str())
    }

    /// Get number of rows
    pub const fn rows(&self) -> usize {
        SCREEN_ROWS
    }

    /// Get number of columns
    pub const fn cols(&self) -> usize {
        SCREEN_COLS
    }

    /// Get the current selection/highlight (first highlighted row)
    ///
    /// Returns (row, start_col, end_col) if any row is highlighted.
    pub fn selection(&self) -> Option<(u8, u8, u8)> {
        for (row, highlight) in self.highlights.iter().enumerate() {
            if let Some((start, end)) = highlight {
                return Some((row as u8, *start, *end));
            }
        }
        None
    }

    /// Get number of rows as u8
    pub const fn rows_u8(&self) -> u8 {
        SCREEN_ROWS as u8
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Screen {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "Screen[");
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                defmt::write!(f, ", ");
            }
            defmt::write!(f, "{}", line.as_str());
        }
        defmt::write!(f, "]");
    }
}
