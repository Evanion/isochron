//! SH1106 OLED Display Driver
//!
//! Driver for 128x64 SH1106-based OLED displays via I2C.
//! Optimized for text display with 6x8 font (21 chars x 8 rows).

use crate::font::FONT_6X8;

/// SH1106 I2C address (typically 0x3C or 0x3D)
const SH1106_ADDR: u8 = 0x3C;

/// Display dimensions
const WIDTH: usize = 128;
const HEIGHT: usize = 64;
const PAGES: usize = HEIGHT / 8;

/// SH1106 commands
#[allow(dead_code)]
mod cmd {
    pub const DISPLAY_OFF: u8 = 0xAE;
    pub const DISPLAY_ON: u8 = 0xAF;
    pub const SET_CONTRAST: u8 = 0x81;
    pub const SET_NORMAL: u8 = 0xA6;
    pub const SET_INVERSE: u8 = 0xA7;
    pub const SET_DISPLAY_OFFSET: u8 = 0xD3;
    pub const SET_COM_PINS: u8 = 0xDA;
    pub const SET_VCOM_DETECT: u8 = 0xDB;
    pub const SET_CLOCK_DIV: u8 = 0xD5;
    pub const SET_PRECHARGE: u8 = 0xD9;
    pub const SET_MUX_RATIO: u8 = 0xA8;
    pub const SET_LOW_COLUMN: u8 = 0x00;
    pub const SET_HIGH_COLUMN: u8 = 0x10;
    pub const SET_PAGE_ADDR: u8 = 0xB0;
    pub const SET_START_LINE: u8 = 0x40;
    pub const SET_SEG_REMAP: u8 = 0xA1;
    pub const SET_COM_SCAN_DEC: u8 = 0xC8;
    pub const SET_CHARGE_PUMP: u8 = 0x8D;
}

/// SH1106 OLED driver
pub struct Sh1106<I2C> {
    i2c: I2C,
    /// Frame buffer (1 bit per pixel, organized as pages)
    buffer: [[u8; WIDTH]; PAGES],
}

impl<I2C> Sh1106<I2C>
where
    I2C: embedded_hal_async::i2c::I2c,
{
    /// Create a new SH1106 driver
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            buffer: [[0; WIDTH]; PAGES],
        }
    }

    /// Initialize the display
    pub async fn init(&mut self) -> Result<(), I2C::Error> {
        // Initialization sequence for SH1106
        let init_cmds: &[u8] = &[
            cmd::DISPLAY_OFF,
            cmd::SET_CLOCK_DIV,
            0x80, // Default clock
            cmd::SET_MUX_RATIO,
            0x3F, // 64 lines
            cmd::SET_DISPLAY_OFFSET,
            0x00,
            cmd::SET_START_LINE | 0x00,
            cmd::SET_CHARGE_PUMP,
            0x14,                  // Enable charge pump
            cmd::SET_SEG_REMAP,    // Flip horizontally
            cmd::SET_COM_SCAN_DEC, // Flip vertically
            cmd::SET_COM_PINS,
            0x12, // Alternative COM config
            cmd::SET_CONTRAST,
            0xCF, // High contrast
            cmd::SET_PRECHARGE,
            0xF1,
            cmd::SET_VCOM_DETECT,
            0x40,
            cmd::SET_NORMAL,
            cmd::DISPLAY_ON,
        ];

        for &c in init_cmds {
            self.command(c).await?;
        }

        Ok(())
    }

    /// Send a command to the display
    async fn command(&mut self, cmd: u8) -> Result<(), I2C::Error> {
        self.i2c.write(SH1106_ADDR, &[0x00, cmd]).await
    }

    /// Clear the frame buffer
    pub async fn clear(&mut self) -> Result<(), I2C::Error> {
        for page in self.buffer.iter_mut() {
            page.fill(0);
        }
        Ok(())
    }

    /// Draw text at the specified position (row 0-7, col 0-20)
    pub async fn draw_text(&mut self, row: u8, col: u8, text: &str) -> Result<(), I2C::Error> {
        if row >= PAGES as u8 {
            return Ok(());
        }

        let page = &mut self.buffer[row as usize];
        let mut x = (col as usize) * 6 + 2; // SH1106 has 2-pixel offset

        for ch in text.chars() {
            if x + 6 > WIDTH {
                break;
            }

            let glyph = get_glyph(ch);
            for i in 0..6 {
                if x + i < WIDTH {
                    page[x + i] = glyph[i];
                }
            }
            x += 6;
        }

        Ok(())
    }

    /// Invert a region of a row (for selection highlighting)
    pub async fn invert_region(
        &mut self,
        row: u8,
        start_col: u8,
        end_col: u8,
    ) -> Result<(), I2C::Error> {
        if row >= PAGES as u8 {
            return Ok(());
        }

        let page = &mut self.buffer[row as usize];
        let start_x = (start_col as usize) * 6 + 2;
        let end_x = ((end_col as usize) * 6 + 2).min(WIDTH);

        for x in start_x..end_x {
            page[x] ^= 0xFF;
        }

        Ok(())
    }

    /// Flush the frame buffer to the display
    pub async fn flush(&mut self) -> Result<(), I2C::Error> {
        for page in 0..PAGES {
            // Set page address
            self.command(cmd::SET_PAGE_ADDR | (page as u8)).await?;
            // Set column address (SH1106 starts at column 2)
            self.command(cmd::SET_LOW_COLUMN | 2).await?;
            self.command(cmd::SET_HIGH_COLUMN | 0).await?;

            // Send page data
            let mut data = [0u8; WIDTH + 1];
            data[0] = 0x40; // Data mode
            data[1..].copy_from_slice(&self.buffer[page]);
            self.i2c.write(SH1106_ADDR, &data).await?;
        }

        Ok(())
    }

    /// Set display contrast (0-255)
    #[allow(dead_code)]
    pub async fn set_contrast(&mut self, contrast: u8) -> Result<(), I2C::Error> {
        self.command(cmd::SET_CONTRAST).await?;
        self.command(contrast).await
    }

    /// Turn display on/off
    #[allow(dead_code)]
    pub async fn set_display_on(&mut self, on: bool) -> Result<(), I2C::Error> {
        if on {
            self.command(cmd::DISPLAY_ON).await
        } else {
            self.command(cmd::DISPLAY_OFF).await
        }
    }

    /// Invert display colors
    #[allow(dead_code)]
    pub async fn set_inverted(&mut self, inverted: bool) -> Result<(), I2C::Error> {
        if inverted {
            self.command(cmd::SET_INVERSE).await
        } else {
            self.command(cmd::SET_NORMAL).await
        }
    }
}

/// Get the 6x8 glyph for a character
fn get_glyph(ch: char) -> &'static [u8; 6] {
    let idx = ch as usize;
    if idx >= 32 && idx < 128 {
        &FONT_6X8[idx - 32]
    } else {
        &FONT_6X8[0] // Space for unknown chars
    }
}
