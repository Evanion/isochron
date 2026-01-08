//! GPIO abstractions for STM32F0
//!
//! Provides GPIO pin management for STM32F0 series chips.

/// GPIO allocator to track pin usage
pub struct GpioAllocator {
    /// Bitmask of allocated GPIO pins (up to 32 pins)
    allocated: u32,
}

impl Default for GpioAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl GpioAllocator {
    /// Create a new GPIO allocator
    pub fn new() -> Self {
        Self { allocated: 0 }
    }

    /// Allocate a GPIO pin
    pub fn allocate(&mut self, pin: u8) -> Result<(), ()> {
        if pin >= 32 {
            return Err(());
        }
        let mask = 1 << pin;
        if self.allocated & mask != 0 {
            return Err(());
        }
        self.allocated |= mask;
        Ok(())
    }

    /// Release a GPIO pin
    pub fn release(&mut self, pin: u8) {
        if pin < 32 {
            self.allocated &= !(1 << pin);
        }
    }

    /// Check if a pin is allocated
    pub fn is_allocated(&self, pin: u8) -> bool {
        if pin >= 32 {
            return false;
        }
        self.allocated & (1 << pin) != 0
    }
}

/// Parse a pin string from config
///
/// Supports formats:
/// - "PA0" -> (Port A, Pin 0, false)
/// - "!PB1" -> (Port B, Pin 1, true/inverted)
pub fn parse_pin_string(s: &str) -> Option<(char, u8, bool)> {
    let s = s.trim();

    let (s, inverted) = if s.starts_with('!') {
        (&s[1..], true)
    } else {
        (s, false)
    };

    if !s.starts_with('P') || s.len() < 3 {
        return None;
    }

    let port = s.chars().nth(1)?;
    if !('A'..='F').contains(&port) {
        return None;
    }

    let pin_str = &s[2..];
    let pin: u8 = pin_str.parse().ok()?;
    if pin > 15 {
        return None;
    }

    Some((port, pin, inverted))
}
