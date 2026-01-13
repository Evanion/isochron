//! Protocol helpers for display communication
//!
//! Provides convenience functions for encoding and sending display commands.

use isochron_protocol::{Frame, FrameError, PicoMessage};

/// Encode a screen to a series of frames
///
/// Returns frames for:
/// 1. Clear screen
/// 2. Text for each non-empty line
/// 3. Invert command if a row is selected
pub fn encode_screen(screen: &super::Screen) -> impl Iterator<Item = Frame> + '_ {
    ScreenEncoder::new(screen)
}

/// Iterator that encodes a screen into frames
struct ScreenEncoder<'a> {
    screen: &'a super::Screen,
    state: EncoderState,
    current_row: u8,
}

#[derive(Clone, Copy)]
enum EncoderState {
    Clear,
    Lines,
    Selection,
    Done,
}

impl<'a> ScreenEncoder<'a> {
    fn new(screen: &'a super::Screen) -> Self {
        Self {
            screen,
            state: EncoderState::Clear,
            current_row: 0,
        }
    }
}

impl<'a> Iterator for ScreenEncoder<'a> {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.state {
                EncoderState::Clear => {
                    self.state = EncoderState::Lines;
                    return PicoMessage::Clear.to_frame().ok();
                }
                EncoderState::Lines => {
                    while self.current_row < 8 {
                        let row = self.current_row;
                        self.current_row += 1;

                        let line = self.screen.get_line(row);
                        if !line.is_empty() {
                            let msg = PicoMessage::Text {
                                row,
                                col: 0,
                                text: line,
                            };
                            return msg.to_frame().ok();
                        }
                    }
                    self.state = EncoderState::Selection;
                }
                EncoderState::Selection => {
                    self.state = EncoderState::Done;
                    if let Some(row) = self.screen.selected_row() {
                        if self.screen.invert_selection() {
                            let msg = PicoMessage::Invert {
                                row,
                                start_col: 0,
                                end_col: 20,
                            };
                            return msg.to_frame().ok();
                        }
                    }
                }
                EncoderState::Done => return None,
            }
        }
    }
}

/// Build a PONG response frame
pub fn pong_frame() -> Result<Frame, FrameError> {
    PicoMessage::Pong.to_frame()
}

// Tests require std feature (not available on embedded target)
#[cfg(all(test, feature = "std"))]
mod tests {
    extern crate alloc;
    use super::*;
    use crate::display::Screen;
    use alloc::vec::Vec;

    #[test]
    fn test_encode_empty_screen() {
        let screen = Screen::new();
        let frames: Vec<_> = encode_screen(&screen).collect();

        // Should just be a clear command
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_encode_screen_with_text() {
        let mut screen = Screen::new();
        screen.set_line(0, "Hello");
        screen.set_line(2, "World");

        let frames: Vec<_> = encode_screen(&screen).collect();

        // Clear + 2 text commands
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn test_encode_screen_with_selection() {
        let mut screen = Screen::new();
        screen.set_line(1, "Selected");
        screen.set_selection(1, true);

        let frames: Vec<_> = encode_screen(&screen).collect();

        // Clear + text + invert
        assert_eq!(frames.len(), 3);
    }
}
