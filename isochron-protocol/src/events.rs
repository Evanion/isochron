//! Input events from the V0 Display encoder

/// Input event values sent from the V0 Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InputEvent {
    /// Encoder rotated clockwise (1 detent)
    EncoderCw,
    /// Encoder rotated counter-clockwise (1 detent)
    EncoderCcw,
    /// Short press (<500 ms)
    EncoderClick,
    /// Long press (>=500 ms)
    EncoderLongPress,
    /// Button released (after long press)
    EncoderRelease,
}

// Wire format values
const EVENT_ENCODER_CW: u8 = 0x01;
const EVENT_ENCODER_CCW: u8 = 0x02;
const EVENT_ENCODER_CLICK: u8 = 0x10;
const EVENT_ENCODER_LONG_PRESS: u8 = 0x11;
const EVENT_ENCODER_RELEASE: u8 = 0x12;

impl InputEvent {
    /// Parse an event from its wire format byte
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            EVENT_ENCODER_CW => Some(InputEvent::EncoderCw),
            EVENT_ENCODER_CCW => Some(InputEvent::EncoderCcw),
            EVENT_ENCODER_CLICK => Some(InputEvent::EncoderClick),
            EVENT_ENCODER_LONG_PRESS => Some(InputEvent::EncoderLongPress),
            EVENT_ENCODER_RELEASE => Some(InputEvent::EncoderRelease),
            _ => None,
        }
    }

    /// Convert to wire format byte
    pub fn to_byte(self) -> u8 {
        match self {
            InputEvent::EncoderCw => EVENT_ENCODER_CW,
            InputEvent::EncoderCcw => EVENT_ENCODER_CCW,
            InputEvent::EncoderClick => EVENT_ENCODER_CLICK,
            InputEvent::EncoderLongPress => EVENT_ENCODER_LONG_PRESS,
            InputEvent::EncoderRelease => EVENT_ENCODER_RELEASE,
        }
    }

    /// Returns true if this is a rotation event
    pub fn is_rotation(&self) -> bool {
        matches!(self, InputEvent::EncoderCw | InputEvent::EncoderCcw)
    }

    /// Returns true if this is a button event
    pub fn is_button(&self) -> bool {
        matches!(
            self,
            InputEvent::EncoderClick | InputEvent::EncoderLongPress | InputEvent::EncoderRelease
        )
    }

    /// Returns the rotation direction as a signed delta (-1, 0, or +1)
    pub fn rotation_delta(&self) -> i8 {
        match self {
            InputEvent::EncoderCw => 1,
            InputEvent::EncoderCcw => -1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_roundtrip() {
        let events = [
            InputEvent::EncoderCw,
            InputEvent::EncoderCcw,
            InputEvent::EncoderClick,
            InputEvent::EncoderLongPress,
            InputEvent::EncoderRelease,
        ];

        for event in events {
            let byte = event.to_byte();
            let parsed = InputEvent::from_byte(byte).unwrap();
            assert_eq!(event, parsed);
        }
    }

    #[test]
    fn test_rotation_delta() {
        assert_eq!(InputEvent::EncoderCw.rotation_delta(), 1);
        assert_eq!(InputEvent::EncoderCcw.rotation_delta(), -1);
        assert_eq!(InputEvent::EncoderClick.rotation_delta(), 0);
    }

    #[test]
    fn test_is_rotation() {
        assert!(InputEvent::EncoderCw.is_rotation());
        assert!(InputEvent::EncoderCcw.is_rotation());
        assert!(!InputEvent::EncoderClick.is_rotation());
    }

    #[test]
    fn test_is_button() {
        assert!(InputEvent::EncoderClick.is_button());
        assert!(InputEvent::EncoderLongPress.is_button());
        assert!(InputEvent::EncoderRelease.is_button());
        assert!(!InputEvent::EncoderCw.is_button());
    }

    #[test]
    fn test_unknown_event() {
        assert!(InputEvent::from_byte(0xFF).is_none());
        assert!(InputEvent::from_byte(0x00).is_none());
    }
}
