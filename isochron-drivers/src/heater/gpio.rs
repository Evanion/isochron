//! GPIO heater output
//!
//! Simple heater control using a GPIO pin (directly or via SSR/MOSFET).

use isochron_core::traits::HeaterOutput;

/// Trait for GPIO pin abstraction
pub trait OutputPin {
    /// Set the pin high
    fn set_high(&mut self);

    /// Set the pin low
    fn set_low(&mut self);

    /// Check if the pin is set high
    fn is_set_high(&self) -> bool;
}

/// GPIO heater output
///
/// Controls a heater via a GPIO pin. The pin can be configured as
/// active-high (default) or active-low.
pub struct GpioHeater<P> {
    pin: P,
    /// If true, heater ON = pin LOW
    inverted: bool,
    /// Current logical state (true = heater on)
    on: bool,
}

impl<P: OutputPin> GpioHeater<P> {
    /// Create a new GPIO heater output
    ///
    /// # Arguments
    /// - `pin`: The GPIO pin to control
    /// - `inverted`: If true, heater is ON when pin is LOW (for active-low SSRs)
    pub fn new(pin: P, inverted: bool) -> Self {
        let mut heater = Self {
            pin,
            inverted,
            on: false,
        };
        // Ensure heater starts off
        heater.set_on(false);
        heater
    }

    /// Create a new GPIO heater with active-high output
    pub fn new_active_high(pin: P) -> Self {
        Self::new(pin, false)
    }

    /// Create a new GPIO heater with active-low output
    pub fn new_active_low(pin: P) -> Self {
        Self::new(pin, true)
    }
}

impl<P: OutputPin> HeaterOutput for GpioHeater<P> {
    fn set_on(&mut self, on: bool) {
        self.on = on;

        if on != self.inverted {
            // Normal: on=true, inverted=false → high
            // Inverted: on=true, inverted=true → low
            self.pin.set_high();
        } else {
            self.pin.set_low();
        }
    }

    fn is_on(&self) -> bool {
        self.on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock GPIO pin for testing
    struct MockPin {
        high: bool,
    }

    impl MockPin {
        fn new() -> Self {
            Self { high: false }
        }
    }

    impl OutputPin for MockPin {
        fn set_high(&mut self) {
            self.high = true;
        }

        fn set_low(&mut self) {
            self.high = false;
        }

        fn is_set_high(&self) -> bool {
            self.high
        }
    }

    #[test]
    fn test_active_high_heater() {
        let pin = MockPin::new();
        let mut heater = GpioHeater::new_active_high(pin);

        // Initially off
        assert!(!heater.is_on());
        assert!(!heater.pin.is_set_high());

        // Turn on
        heater.set_on(true);
        assert!(heater.is_on());
        assert!(heater.pin.is_set_high());

        // Turn off
        heater.set_on(false);
        assert!(!heater.is_on());
        assert!(!heater.pin.is_set_high());
    }

    #[test]
    fn test_active_low_heater() {
        let pin = MockPin::new();
        let mut heater = GpioHeater::new_active_low(pin);

        // Initially off (pin is high for active-low)
        assert!(!heater.is_on());
        assert!(heater.pin.is_set_high());

        // Turn on (pin goes low for active-low)
        heater.set_on(true);
        assert!(heater.is_on());
        assert!(!heater.pin.is_set_high());

        // Turn off (pin goes high for active-low)
        heater.set_on(false);
        assert!(!heater.is_on());
        assert!(heater.pin.is_set_high());
    }

    #[test]
    fn test_heater_trait() {
        let pin = MockPin::new();
        let mut heater = GpioHeater::new_active_high(pin);

        // Use trait method through concrete type
        fn check_heater<H: HeaterOutput>(h: &mut H) {
            assert!(!h.is_on());
            h.set_on(true);
            assert!(h.is_on());
        }

        check_heater(&mut heater);
    }
}
