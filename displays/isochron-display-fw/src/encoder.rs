//! Rotary Encoder Handler
//!
//! Decodes quadrature encoder signals and generates navigation events.
//! Uses state machine for reliable decoding with noise rejection.

use embassy_stm32::gpio::Input;
use embassy_time::{Duration, Timer};

use isochron_protocol::InputEvent;

/// Encoder state machine states
#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    CwStep1,
    CwStep2,
    CcwStep1,
    CcwStep2,
}

/// Quadrature encoder handler
pub struct Encoder<'d> {
    a: Input<'d>,
    b: Input<'d>,
    state: State,
    last_a: bool,
    last_b: bool,
}

impl<'d> Encoder<'d> {
    /// Create a new encoder handler
    pub fn new(a: Input<'d>, b: Input<'d>) -> Self {
        let last_a = a.is_high();
        let last_b = b.is_high();

        Self {
            a,
            b,
            state: State::Idle,
            last_a,
            last_b,
        }
    }

    /// Poll for encoder events
    ///
    /// Returns an input event if rotation is detected.
    /// Should be called frequently (every 1-5ms).
    pub async fn poll(&mut self) -> Option<InputEvent> {
        // Small delay between polls
        Timer::after(Duration::from_millis(2)).await;

        let a = self.a.is_high();
        let b = self.b.is_high();

        // No change
        if a == self.last_a && b == self.last_b {
            return None;
        }

        let event = self.decode(a, b);

        self.last_a = a;
        self.last_b = b;

        event
    }

    /// Decode encoder state using state machine
    ///
    /// Quadrature encoding:
    /// CW:  A leads B (A changes first when rotating clockwise)
    /// CCW: B leads A (B changes first when rotating counter-clockwise)
    ///
    /// State transitions for CW rotation:
    /// Idle (1,1) -> CwStep1 (0,1) -> CwStep2 (0,0) -> Output ScrollUp -> Idle (1,0 or 1,1)
    ///
    /// State transitions for CCW rotation:
    /// Idle (1,1) -> CcwStep1 (1,0) -> CcwStep2 (0,0) -> Output ScrollDown -> Idle (0,1 or 1,1)
    fn decode(&mut self, a: bool, b: bool) -> Option<InputEvent> {
        match self.state {
            State::Idle => {
                if !a && b {
                    // A fell first -> CW direction
                    self.state = State::CwStep1;
                } else if a && !b {
                    // B fell first -> CCW direction
                    self.state = State::CcwStep1;
                }
                None
            }
            State::CwStep1 => {
                if !a && !b {
                    // Both low -> continue CW
                    self.state = State::CwStep2;
                } else if a && b {
                    // Back to idle (noise/bounce)
                    self.state = State::Idle;
                }
                None
            }
            State::CwStep2 => {
                if a || b {
                    // Either went high -> complete CW step
                    self.state = State::Idle;
                    return Some(InputEvent::EncoderCw);
                }
                None
            }
            State::CcwStep1 => {
                if !a && !b {
                    // Both low -> continue CCW
                    self.state = State::CcwStep2;
                } else if a && b {
                    // Back to idle (noise/bounce)
                    self.state = State::Idle;
                }
                None
            }
            State::CcwStep2 => {
                if a || b {
                    // Either went high -> complete CCW step
                    self.state = State::Idle;
                    return Some(InputEvent::EncoderCcw);
                }
                None
            }
        }
    }
}
