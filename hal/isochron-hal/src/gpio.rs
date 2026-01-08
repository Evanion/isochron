//! GPIO pin abstractions
//!
//! Provides traits for digital input and output pins that can be implemented
//! by chip-specific HALs.

/// Digital output pin
///
/// Implementations should handle the actual hardware register manipulation
/// for the specific chip.
pub trait OutputPin {
    /// Set the pin high (logic 1)
    fn set_high(&mut self);

    /// Set the pin low (logic 0)
    fn set_low(&mut self);

    /// Toggle the pin state
    fn toggle(&mut self);

    /// Set the pin to a specific state
    fn set_state(&mut self, high: bool) {
        if high {
            self.set_high();
        } else {
            self.set_low();
        }
    }

    /// Check if the pin is currently set high
    fn is_set_high(&self) -> bool;

    /// Check if the pin is currently set low
    fn is_set_low(&self) -> bool {
        !self.is_set_high()
    }
}

/// Digital input pin
///
/// Implementations should handle the actual hardware register reading
/// for the specific chip.
pub trait InputPin {
    /// Check if the pin reads high (logic 1)
    fn is_high(&self) -> bool;

    /// Check if the pin reads low (logic 0)
    fn is_low(&self) -> bool {
        !self.is_high()
    }
}

/// Pin that can be used for both input and output
///
/// Some applications need to read the state of an output pin or
/// dynamically switch between input and output modes.
pub trait IoPin: OutputPin + InputPin {}

// Blanket implementation for types that implement both traits
impl<T: OutputPin + InputPin> IoPin for T {}
