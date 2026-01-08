//! Frame encoding and decoding for the V0 Display protocol.
//!
//! Frame format:
//! - START (1 byte): 0xAA synchronization byte
//! - LENGTH (1 byte): payload length (0-250)
//! - TYPE (1 byte): message type identifier
//! - PAYLOAD (0-250 bytes): type-specific data
//! - CHECKSUM (1 byte): XOR of LENGTH, TYPE, and all PAYLOAD bytes

use heapless::Vec;

/// Frame synchronization byte
pub const FRAME_START: u8 = 0xAA;

/// Maximum payload size in bytes
pub const MAX_PAYLOAD_SIZE: usize = 250;

/// Maximum complete frame size (START + LENGTH + TYPE + MAX_PAYLOAD + CHECKSUM)
pub const MAX_FRAME_SIZE: usize = 1 + 1 + 1 + MAX_PAYLOAD_SIZE + 1;

/// Errors that can occur during frame parsing or encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FrameError {
    /// Payload exceeds maximum allowed size
    PayloadTooLarge,
    /// Checksum mismatch
    InvalidChecksum,
    /// Frame is incomplete (need more bytes)
    Incomplete,
    /// Invalid frame structure
    InvalidFrame,
    /// Buffer too small for encoding
    BufferTooSmall,
}

/// A parsed or constructed frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Message type identifier
    pub msg_type: u8,
    /// Payload data
    pub payload: Vec<u8, MAX_PAYLOAD_SIZE>,
}

impl Frame {
    /// Create a new frame with the given message type and payload
    pub fn new(msg_type: u8, payload: &[u8]) -> Result<Self, FrameError> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooLarge);
        }

        let mut payload_vec = Vec::new();
        payload_vec
            .extend_from_slice(payload)
            .map_err(|_| FrameError::PayloadTooLarge)?;

        Ok(Self {
            msg_type,
            payload: payload_vec,
        })
    }

    /// Create a frame with no payload
    pub fn empty(msg_type: u8) -> Self {
        Self {
            msg_type,
            payload: Vec::new(),
        }
    }

    /// Calculate checksum for frame data
    fn calculate_checksum(length: u8, msg_type: u8, payload: &[u8]) -> u8 {
        let mut checksum = length ^ msg_type;
        for &byte in payload {
            checksum ^= byte;
        }
        checksum
    }

    /// Encode this frame into a byte buffer
    ///
    /// Returns the number of bytes written
    pub fn encode(&self, buffer: &mut [u8]) -> Result<usize, FrameError> {
        let frame_len = 4 + self.payload.len(); // START + LENGTH + TYPE + payload + CHECKSUM
        if buffer.len() < frame_len {
            return Err(FrameError::BufferTooSmall);
        }

        let length = self.payload.len() as u8;
        let checksum = Self::calculate_checksum(length, self.msg_type, &self.payload);

        buffer[0] = FRAME_START;
        buffer[1] = length;
        buffer[2] = self.msg_type;
        buffer[3..3 + self.payload.len()].copy_from_slice(&self.payload);
        buffer[3 + self.payload.len()] = checksum;

        Ok(frame_len)
    }

    /// Encode this frame into a heapless Vec
    pub fn encode_to_vec(&self) -> Result<Vec<u8, MAX_FRAME_SIZE>, FrameError> {
        let mut buffer = [0u8; MAX_FRAME_SIZE];
        let len = self.encode(&mut buffer)?;
        let mut vec = Vec::new();
        vec.extend_from_slice(&buffer[..len])
            .map_err(|_| FrameError::BufferTooSmall)?;
        Ok(vec)
    }
}

/// State machine for parsing incoming frames
#[derive(Debug, Clone)]
pub struct FrameParser {
    state: ParseState,
    buffer: Vec<u8, MAX_PAYLOAD_SIZE>,
    expected_length: u8,
    msg_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    /// Waiting for START byte
    WaitingForStart,
    /// Got START, waiting for LENGTH
    WaitingForLength,
    /// Got LENGTH, waiting for TYPE
    WaitingForType,
    /// Reading payload bytes
    ReadingPayload,
    /// Waiting for CHECKSUM
    WaitingForChecksum,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    /// Create a new frame parser
    pub fn new() -> Self {
        Self {
            state: ParseState::WaitingForStart,
            buffer: Vec::new(),
            expected_length: 0,
            msg_type: 0,
        }
    }

    /// Reset the parser state
    pub fn reset(&mut self) {
        self.state = ParseState::WaitingForStart;
        self.buffer.clear();
        self.expected_length = 0;
        self.msg_type = 0;
    }

    /// Feed a single byte to the parser
    ///
    /// Returns `Ok(Some(frame))` when a complete valid frame is parsed,
    /// `Ok(None)` when more bytes are needed, or `Err` on parse error.
    pub fn feed(&mut self, byte: u8) -> Result<Option<Frame>, FrameError> {
        match self.state {
            ParseState::WaitingForStart => {
                if byte == FRAME_START {
                    self.state = ParseState::WaitingForLength;
                }
                // Silently ignore non-START bytes while waiting
                Ok(None)
            }
            ParseState::WaitingForLength => {
                if byte > MAX_PAYLOAD_SIZE as u8 {
                    self.reset();
                    return Err(FrameError::InvalidFrame);
                }
                self.expected_length = byte;
                self.state = ParseState::WaitingForType;
                Ok(None)
            }
            ParseState::WaitingForType => {
                self.msg_type = byte;
                if self.expected_length == 0 {
                    self.state = ParseState::WaitingForChecksum;
                } else {
                    self.buffer.clear();
                    self.state = ParseState::ReadingPayload;
                }
                Ok(None)
            }
            ParseState::ReadingPayload => {
                // This should not fail since we check expected_length
                let _ = self.buffer.push(byte);
                if self.buffer.len() == self.expected_length as usize {
                    self.state = ParseState::WaitingForChecksum;
                }
                Ok(None)
            }
            ParseState::WaitingForChecksum => {
                let expected_checksum =
                    Frame::calculate_checksum(self.expected_length, self.msg_type, &self.buffer);

                if byte != expected_checksum {
                    self.reset();
                    return Err(FrameError::InvalidChecksum);
                }

                let frame = Frame {
                    msg_type: self.msg_type,
                    payload: self.buffer.clone(),
                };

                self.reset();
                Ok(Some(frame))
            }
        }
    }

    /// Feed multiple bytes to the parser
    ///
    /// Returns the first complete frame found, if any.
    /// Remaining bytes after a complete frame are not consumed.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Result<Option<Frame>, FrameError> {
        for &byte in bytes {
            if let Some(frame) = self.feed(byte)? {
                return Ok(Some(frame));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_empty_payload() {
        let frame = Frame::empty(0x20); // CLEAR command
        let mut buffer = [0u8; 10];
        let len = frame.encode(&mut buffer).unwrap();

        assert_eq!(len, 4);
        assert_eq!(buffer[0], FRAME_START);
        assert_eq!(buffer[1], 0); // length
        assert_eq!(buffer[2], 0x20); // type
        assert_eq!(buffer[3], 0x20); // checksum (0 ^ 0x20 = 0x20)
    }

    #[test]
    fn test_frame_encode_with_payload() {
        let frame = Frame::new(0x21, &[0, 0, 5, b'H', b'e', b'l', b'l', b'o']).unwrap();
        let mut buffer = [0u8; 20];
        let len = frame.encode(&mut buffer).unwrap();

        assert_eq!(len, 12);
        assert_eq!(buffer[0], FRAME_START);
        assert_eq!(buffer[1], 8); // length
        assert_eq!(buffer[2], 0x21); // type
                                     // payload starts at buffer[3]
        assert_eq!(buffer[3], 0); // row
        assert_eq!(buffer[4], 0); // col
        assert_eq!(buffer[5], 5); // string length
    }

    #[test]
    fn test_frame_roundtrip() {
        let original = Frame::new(0x21, &[1, 2, 3, 4, 5]).unwrap();
        let encoded = original.encode_to_vec().unwrap();

        let mut parser = FrameParser::new();
        let parsed = parser.feed_bytes(&encoded).unwrap().unwrap();

        assert_eq!(parsed.msg_type, original.msg_type);
        assert_eq!(parsed.payload, original.payload);
    }

    #[test]
    fn test_parser_invalid_checksum() {
        let frame = Frame::empty(0x20);
        let mut encoded = frame.encode_to_vec().unwrap();
        // Corrupt the checksum
        let last_idx = encoded.len() - 1;
        encoded[last_idx] ^= 0xFF;

        let mut parser = FrameParser::new();
        let result = parser.feed_bytes(&encoded);
        assert_eq!(result, Err(FrameError::InvalidChecksum));
    }

    #[test]
    fn test_parser_resync_after_garbage() {
        let frame = Frame::empty(0x24); // PONG
        let encoded = frame.encode_to_vec().unwrap();

        // Prepend garbage bytes
        let mut data = Vec::<u8, 20>::new();
        data.extend_from_slice(&[0x00, 0xFF, 0x12, 0x34]).unwrap();
        data.extend_from_slice(&encoded).unwrap();

        let mut parser = FrameParser::new();
        let parsed = parser.feed_bytes(&data).unwrap().unwrap();

        assert_eq!(parsed.msg_type, 0x24);
    }

    #[test]
    fn test_payload_too_large() {
        let large_payload = [0u8; MAX_PAYLOAD_SIZE + 1];
        let result = Frame::new(0x21, &large_payload);
        assert_eq!(result, Err(FrameError::PayloadTooLarge));
    }
}
