//! UART peripheral allocation
//!
//! RP2040 has two UART peripherals (UART0 and UART1).
//! This module tracks their usage.

/// UART peripheral identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum UartId {
    Uart0,
    Uart1,
}

/// UART allocation state
pub struct UartAllocator {
    uart0_allocated: bool,
    uart1_allocated: bool,
}

impl Default for UartAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl UartAllocator {
    /// Create a new UART allocator
    pub fn new() -> Self {
        Self {
            uart0_allocated: false,
            uart1_allocated: false,
        }
    }

    /// Allocate a UART peripheral
    pub fn allocate(&mut self, id: UartId) -> Result<(), ()> {
        match id {
            UartId::Uart0 => {
                if self.uart0_allocated {
                    Err(())
                } else {
                    self.uart0_allocated = true;
                    Ok(())
                }
            }
            UartId::Uart1 => {
                if self.uart1_allocated {
                    Err(())
                } else {
                    self.uart1_allocated = true;
                    Ok(())
                }
            }
        }
    }

    /// Release a UART peripheral
    pub fn release(&mut self, id: UartId) {
        match id {
            UartId::Uart0 => self.uart0_allocated = false,
            UartId::Uart1 => self.uart1_allocated = false,
        }
    }

    /// Check if a UART is allocated
    pub fn is_allocated(&self, id: UartId) -> bool {
        match id {
            UartId::Uart0 => self.uart0_allocated,
            UartId::Uart1 => self.uart1_allocated,
        }
    }
}

/// Determine which UART can use a given GPIO pin
///
/// RP2040 has specific pin mappings for each UART.
pub fn gpio_to_uart(gpio: u8) -> Option<UartId> {
    // UART0: GPIO 0/1, 12/13, 16/17
    // UART1: GPIO 4/5, 8/9, 20/21, 24/25
    match gpio {
        0 | 1 | 12 | 13 | 16 | 17 => Some(UartId::Uart0),
        4 | 5 | 8 | 9 | 20 | 21 | 24 | 25 => Some(UartId::Uart1),
        _ => None,
    }
}
