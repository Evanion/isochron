//! Message types for the V0 Display protocol
//!
//! Message types are divided into two categories:
//! - Display → Pico: Input events, heartbeat requests
//! - Pico → Display: Screen commands, heartbeat responses

use crate::frame::{Frame, FrameError, MAX_PAYLOAD_SIZE};
use crate::events::InputEvent;
use heapless::Vec;

// Message type IDs: Display → Pico
pub const MSG_INPUT: u8 = 0x01;
pub const MSG_PING: u8 = 0x02;
pub const MSG_ACK: u8 = 0x03;

// Message type IDs: Pico → Display
pub const MSG_CLEAR: u8 = 0x20;
pub const MSG_TEXT: u8 = 0x21;
pub const MSG_INVERT: u8 = 0x22;
pub const MSG_HLINE: u8 = 0x23;
pub const MSG_PONG: u8 = 0x24;
pub const MSG_RESET: u8 = 0x2F;

/// Display dimensions
pub const DISPLAY_ROWS: u8 = 8;
pub const DISPLAY_COLS: u8 = 21;

/// Messages from the Pico to the Display
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PicoMessage<'a> {
    /// Clear the entire screen
    Clear,
    /// Draw text at a position
    Text {
        row: u8,
        col: u8,
        text: &'a str,
    },
    /// Invert a region (for selection highlight)
    Invert {
        row: u8,
        start_col: u8,
        end_col: u8,
    },
    /// Draw a horizontal line
    HLine {
        row: u8,
        start_col: u8,
        end_col: u8,
    },
    /// Heartbeat response
    Pong,
    /// Reset display to boot state
    Reset,
}

impl<'a> PicoMessage<'a> {
    /// Encode this message into a frame
    pub fn to_frame(&self) -> Result<Frame, FrameError> {
        match self {
            PicoMessage::Clear => Ok(Frame::empty(MSG_CLEAR)),
            PicoMessage::Text { row, col, text } => {
                // Payload: [row][col][len][chars...]
                let text_bytes = text.as_bytes();
                let len = text_bytes.len().min(DISPLAY_COLS as usize);

                let mut payload = Vec::<u8, MAX_PAYLOAD_SIZE>::new();
                payload.push(*row).map_err(|_| FrameError::PayloadTooLarge)?;
                payload.push(*col).map_err(|_| FrameError::PayloadTooLarge)?;
                payload
                    .push(len as u8)
                    .map_err(|_| FrameError::PayloadTooLarge)?;
                payload
                    .extend_from_slice(&text_bytes[..len])
                    .map_err(|_| FrameError::PayloadTooLarge)?;

                Frame::new(MSG_TEXT, &payload)
            }
            PicoMessage::Invert {
                row,
                start_col,
                end_col,
            } => Frame::new(MSG_INVERT, &[*row, *start_col, *end_col]),
            PicoMessage::HLine {
                row,
                start_col,
                end_col,
            } => Frame::new(MSG_HLINE, &[*row, *start_col, *end_col]),
            PicoMessage::Pong => Ok(Frame::empty(MSG_PONG)),
            PicoMessage::Reset => Ok(Frame::empty(MSG_RESET)),
        }
    }
}

/// Commands parsed from display-originated frames
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayCommand {
    /// User input event
    Input(InputEvent),
    /// Heartbeat request
    Ping,
    /// Acknowledgement of a received command
    Ack { seq: u8 },
}

impl DisplayCommand {
    /// Parse a command from a frame
    pub fn from_frame(frame: &Frame) -> Result<Self, FrameError> {
        match frame.msg_type {
            MSG_INPUT => {
                if frame.payload.is_empty() {
                    return Err(FrameError::InvalidFrame);
                }
                let event = InputEvent::from_byte(frame.payload[0])
                    .ok_or(FrameError::InvalidFrame)?;
                Ok(DisplayCommand::Input(event))
            }
            MSG_PING => Ok(DisplayCommand::Ping),
            MSG_ACK => {
                if frame.payload.is_empty() {
                    return Err(FrameError::InvalidFrame);
                }
                Ok(DisplayCommand::Ack {
                    seq: frame.payload[0],
                })
            }
            _ => Err(FrameError::InvalidFrame),
        }
    }

    /// Encode this command into a frame (for testing or simulation)
    pub fn to_frame(&self) -> Result<Frame, FrameError> {
        match self {
            DisplayCommand::Input(event) => Frame::new(MSG_INPUT, &[event.to_byte()]),
            DisplayCommand::Ping => Ok(Frame::empty(MSG_PING)),
            DisplayCommand::Ack { seq } => Frame::new(MSG_ACK, &[*seq]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pico_message_clear() {
        let msg = PicoMessage::Clear;
        let frame = msg.to_frame().unwrap();
        assert_eq!(frame.msg_type, MSG_CLEAR);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn test_pico_message_text() {
        let msg = PicoMessage::Text {
            row: 0,
            col: 0,
            text: "Hello",
        };
        let frame = msg.to_frame().unwrap();
        assert_eq!(frame.msg_type, MSG_TEXT);
        assert_eq!(frame.payload[0], 0); // row
        assert_eq!(frame.payload[1], 0); // col
        assert_eq!(frame.payload[2], 5); // len
        assert_eq!(&frame.payload[3..8], b"Hello");
    }

    #[test]
    fn test_pico_message_invert() {
        let msg = PicoMessage::Invert {
            row: 2,
            start_col: 0,
            end_col: 20,
        };
        let frame = msg.to_frame().unwrap();
        assert_eq!(frame.msg_type, MSG_INVERT);
        assert_eq!(frame.payload[0], 2);
        assert_eq!(frame.payload[1], 0);
        assert_eq!(frame.payload[2], 20);
    }

    #[test]
    fn test_display_command_input() {
        let frame = Frame::new(MSG_INPUT, &[0x01]).unwrap(); // ENCODER_CW
        let cmd = DisplayCommand::from_frame(&frame).unwrap();
        assert_eq!(cmd, DisplayCommand::Input(InputEvent::EncoderCw));
    }

    #[test]
    fn test_display_command_ping() {
        let frame = Frame::empty(MSG_PING);
        let cmd = DisplayCommand::from_frame(&frame).unwrap();
        assert_eq!(cmd, DisplayCommand::Ping);
    }

    #[test]
    fn test_display_command_roundtrip() {
        let original = DisplayCommand::Input(InputEvent::EncoderClick);
        let frame = original.to_frame().unwrap();
        let parsed = DisplayCommand::from_frame(&frame).unwrap();
        assert_eq!(original, parsed);
    }
}
