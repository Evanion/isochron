//! GPIO allocation and management
//!
//! Tracks which GPIO pins are in use to prevent conflicts.

use heapless::FnvIndexSet;

/// Maximum number of GPIO pins on RP2040
pub const GPIO_COUNT: usize = 30;

/// GPIO allocator to track pin usage
pub struct GpioAllocator {
    /// Set of allocated GPIO pins
    allocated: FnvIndexSet<u8, 32>,
}

impl Default for GpioAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl GpioAllocator {
    /// Create a new GPIO allocator
    pub fn new() -> Self {
        Self {
            allocated: FnvIndexSet::new(),
        }
    }

    /// Allocate a GPIO pin
    ///
    /// Returns `Ok(())` if the pin was successfully allocated,
    /// or `Err(())` if the pin is already in use.
    pub fn allocate(&mut self, pin: u8) -> Result<(), ()> {
        if pin >= GPIO_COUNT as u8 {
            return Err(());
        }
        if self.allocated.contains(&pin) {
            return Err(());
        }
        self.allocated.insert(pin).map_err(|_| ())?;
        Ok(())
    }

    /// Release a GPIO pin
    pub fn release(&mut self, pin: u8) {
        self.allocated.remove(&pin);
    }

    /// Check if a pin is allocated
    pub fn is_allocated(&self, pin: u8) -> bool {
        self.allocated.contains(&pin)
    }

    /// Get the number of allocated pins
    pub fn allocated_count(&self) -> usize {
        self.allocated.len()
    }
}

/// Parse a pin string from config
///
/// Supports formats:
/// - "gpio11" -> (11, false)
/// - "!gpio12" -> (12, true) (inverted/active-low)
/// - "^gpio4" -> (4, false) + pull-up flag (handled separately)
pub fn parse_pin_string(s: &str) -> Option<(u8, bool)> {
    let s = s.trim();

    let (s, inverted) = if s.starts_with('!') {
        (&s[1..], true)
    } else {
        (s, false)
    };

    let s = if s.starts_with('^') { &s[1..] } else { s };

    if !s.starts_with("gpio") {
        return None;
    }

    let num_str = &s[4..];
    let pin: u8 = num_str.parse().ok()?;

    if pin >= GPIO_COUNT as u8 {
        return None;
    }

    Some((pin, inverted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocator() {
        let mut alloc = GpioAllocator::new();

        assert!(alloc.allocate(11).is_ok());
        assert!(alloc.is_allocated(11));

        // Can't allocate same pin twice
        assert!(alloc.allocate(11).is_err());

        // Can allocate different pin
        assert!(alloc.allocate(12).is_ok());

        // Release and re-allocate
        alloc.release(11);
        assert!(!alloc.is_allocated(11));
        assert!(alloc.allocate(11).is_ok());
    }

    #[test]
    fn test_parse_pin_string() {
        assert_eq!(parse_pin_string("gpio11"), Some((11, false)));
        assert_eq!(parse_pin_string("!gpio12"), Some((12, true)));
        assert_eq!(parse_pin_string("^gpio4"), Some((4, false)));
        assert_eq!(parse_pin_string("gpio0"), Some((0, false)));
        assert_eq!(parse_pin_string("gpio29"), Some((29, false)));

        // Invalid
        assert_eq!(parse_pin_string("gpio30"), None);
        assert_eq!(parse_pin_string("pin11"), None);
        assert_eq!(parse_pin_string(""), None);
    }
}
