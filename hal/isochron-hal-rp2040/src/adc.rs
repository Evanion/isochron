//! ADC channel management
//!
//! RP2040 has a single ADC with 5 channels:
//! - ADC0: GPIO26
//! - ADC1: GPIO27
//! - ADC2: GPIO28
//! - ADC3: GPIO29
//! - ADC4: Internal temperature sensor

/// ADC channel identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AdcChannel {
    /// ADC0 on GPIO26
    Adc0,
    /// ADC1 on GPIO27
    Adc1,
    /// ADC2 on GPIO28
    Adc2,
    /// ADC3 on GPIO29
    Adc3,
    /// Internal temperature sensor
    Temperature,
}

impl AdcChannel {
    /// Get the GPIO pin for this ADC channel
    pub fn gpio(&self) -> Option<u8> {
        match self {
            AdcChannel::Adc0 => Some(26),
            AdcChannel::Adc1 => Some(27),
            AdcChannel::Adc2 => Some(28),
            AdcChannel::Adc3 => Some(29),
            AdcChannel::Temperature => None,
        }
    }

    /// Get ADC channel from GPIO pin
    pub fn from_gpio(gpio: u8) -> Option<Self> {
        match gpio {
            26 => Some(AdcChannel::Adc0),
            27 => Some(AdcChannel::Adc1),
            28 => Some(AdcChannel::Adc2),
            29 => Some(AdcChannel::Adc3),
            _ => None,
        }
    }
}

/// ADC allocator
pub struct AdcAllocator {
    allocated: [bool; 5],
}

impl Default for AdcAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl AdcAllocator {
    /// Create a new ADC allocator
    pub fn new() -> Self {
        Self {
            allocated: [false; 5],
        }
    }

    /// Allocate an ADC channel
    pub fn allocate(&mut self, channel: AdcChannel) -> Result<(), ()> {
        let idx = channel as usize;
        if self.allocated[idx] {
            Err(())
        } else {
            self.allocated[idx] = true;
            Ok(())
        }
    }

    /// Release an ADC channel
    pub fn release(&mut self, channel: AdcChannel) {
        self.allocated[channel as usize] = false;
    }

    /// Check if a channel is allocated
    pub fn is_allocated(&self, channel: AdcChannel) -> bool {
        self.allocated[channel as usize]
    }
}
