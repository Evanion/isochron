//! Dynamic pin allocation for config-driven hardware setup
//!
//! Provides a way to get GPIO pins by number at runtime, enabling
//! Klipper-style config-driven pin assignment.

use embassy_rp::gpio::AnyPin;
use embassy_rp::Peri;
use embassy_rp::Peripherals;

/// Macro to take a pin by number from peripherals
///
/// Usage:
/// ```ignore
/// let dir_pin = take_pin!(p, 10); // Takes p.PIN_10 as Peri<AnyPin>
/// ```
#[macro_export]
macro_rules! take_pin {
    ($p:expr, 0) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_0) };
    ($p:expr, 1) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_1) };
    ($p:expr, 2) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_2) };
    ($p:expr, 3) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_3) };
    ($p:expr, 4) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_4) };
    ($p:expr, 5) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_5) };
    ($p:expr, 6) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_6) };
    ($p:expr, 7) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_7) };
    ($p:expr, 8) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_8) };
    ($p:expr, 9) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_9) };
    ($p:expr, 10) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_10) };
    ($p:expr, 11) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_11) };
    ($p:expr, 12) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_12) };
    ($p:expr, 13) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_13) };
    ($p:expr, 14) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_14) };
    ($p:expr, 15) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_15) };
    ($p:expr, 16) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_16) };
    ($p:expr, 17) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_17) };
    ($p:expr, 18) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_18) };
    ($p:expr, 19) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_19) };
    ($p:expr, 20) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_20) };
    ($p:expr, 21) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_21) };
    ($p:expr, 22) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_22) };
    ($p:expr, 23) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_23) };
    ($p:expr, 24) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_24) };
    ($p:expr, 25) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_25) };
    ($p:expr, 26) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_26) };
    ($p:expr, 27) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_27) };
    ($p:expr, 28) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_28) };
    ($p:expr, 29) => { embassy_rp::Peri::<embassy_rp::gpio::AnyPin>::from($p.PIN_29) };
}

/// Error when requesting a pin
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PinError {
    /// Pin number out of range (0-29 valid)
    InvalidPin,
    /// Pin already taken
    AlreadyTaken,
    /// Pin reserved for special function
    Reserved,
}

/// Pin bank that holds all GPIO pins and allows taking them by number
///
/// This enables config-driven pin assignment where pin numbers come from
/// a TOML config file rather than being hardcoded.
pub struct PinBank {
    pins: [Option<Peri<'static, AnyPin>>; 30],
}

impl PinBank {
    /// Create a new pin bank from peripherals
    ///
    /// Takes ownership of all GPIO pins from the Peripherals struct.
    /// After this call, pins must be obtained through `take()`.
    pub fn new(p: &mut PinBankPeripherals) -> Self {
        Self {
            pins: [
                Some(p.pin0.take().unwrap().into()),
                Some(p.pin1.take().unwrap().into()),
                Some(p.pin2.take().unwrap().into()),
                Some(p.pin3.take().unwrap().into()),
                Some(p.pin4.take().unwrap().into()),
                Some(p.pin5.take().unwrap().into()),
                Some(p.pin6.take().unwrap().into()),
                Some(p.pin7.take().unwrap().into()),
                Some(p.pin8.take().unwrap().into()),
                Some(p.pin9.take().unwrap().into()),
                Some(p.pin10.take().unwrap().into()),
                Some(p.pin11.take().unwrap().into()),
                Some(p.pin12.take().unwrap().into()),
                Some(p.pin13.take().unwrap().into()),
                Some(p.pin14.take().unwrap().into()),
                Some(p.pin15.take().unwrap().into()),
                Some(p.pin16.take().unwrap().into()),
                Some(p.pin17.take().unwrap().into()),
                Some(p.pin18.take().unwrap().into()),
                Some(p.pin19.take().unwrap().into()),
                Some(p.pin20.take().unwrap().into()),
                Some(p.pin21.take().unwrap().into()),
                Some(p.pin22.take().unwrap().into()),
                Some(p.pin23.take().unwrap().into()),
                Some(p.pin24.take().unwrap().into()),
                Some(p.pin25.take().unwrap().into()),
                Some(p.pin26.take().unwrap().into()),
                Some(p.pin27.take().unwrap().into()),
                Some(p.pin28.take().unwrap().into()),
                Some(p.pin29.take().unwrap().into()),
            ],
        }
    }

    /// Take a pin by number
    ///
    /// Returns the pin if available, or an error if:
    /// - Pin number is invalid (>= 30)
    /// - Pin was already taken
    pub fn take(&mut self, pin_num: u8) -> Result<Peri<'static, AnyPin>, PinError> {
        if pin_num >= 30 {
            return Err(PinError::InvalidPin);
        }
        self.pins[pin_num as usize]
            .take()
            .ok_or(PinError::AlreadyTaken)
    }

    /// Check if a pin is available
    pub fn is_available(&self, pin_num: u8) -> bool {
        if pin_num >= 30 {
            return false;
        }
        self.pins[pin_num as usize].is_some()
    }

    /// Return a pin to the bank
    ///
    /// Allows re-using a pin after it's no longer needed.
    pub fn return_pin(&mut self, pin_num: u8, pin: Peri<'static, AnyPin>) {
        if pin_num < 30 {
            self.pins[pin_num as usize] = Some(pin);
        }
    }
}

/// Peripherals needed for PinBank
///
/// This struct holds the GPIO pins that will be moved into the PinBank.
/// Using Option allows taking pins individually without consuming the whole Peripherals.
pub struct PinBankPeripherals {
    pub pin0: Option<Peri<'static, embassy_rp::peripherals::PIN_0>>,
    pub pin1: Option<Peri<'static, embassy_rp::peripherals::PIN_1>>,
    pub pin2: Option<Peri<'static, embassy_rp::peripherals::PIN_2>>,
    pub pin3: Option<Peri<'static, embassy_rp::peripherals::PIN_3>>,
    pub pin4: Option<Peri<'static, embassy_rp::peripherals::PIN_4>>,
    pub pin5: Option<Peri<'static, embassy_rp::peripherals::PIN_5>>,
    pub pin6: Option<Peri<'static, embassy_rp::peripherals::PIN_6>>,
    pub pin7: Option<Peri<'static, embassy_rp::peripherals::PIN_7>>,
    pub pin8: Option<Peri<'static, embassy_rp::peripherals::PIN_8>>,
    pub pin9: Option<Peri<'static, embassy_rp::peripherals::PIN_9>>,
    pub pin10: Option<Peri<'static, embassy_rp::peripherals::PIN_10>>,
    pub pin11: Option<Peri<'static, embassy_rp::peripherals::PIN_11>>,
    pub pin12: Option<Peri<'static, embassy_rp::peripherals::PIN_12>>,
    pub pin13: Option<Peri<'static, embassy_rp::peripherals::PIN_13>>,
    pub pin14: Option<Peri<'static, embassy_rp::peripherals::PIN_14>>,
    pub pin15: Option<Peri<'static, embassy_rp::peripherals::PIN_15>>,
    pub pin16: Option<Peri<'static, embassy_rp::peripherals::PIN_16>>,
    pub pin17: Option<Peri<'static, embassy_rp::peripherals::PIN_17>>,
    pub pin18: Option<Peri<'static, embassy_rp::peripherals::PIN_18>>,
    pub pin19: Option<Peri<'static, embassy_rp::peripherals::PIN_19>>,
    pub pin20: Option<Peri<'static, embassy_rp::peripherals::PIN_20>>,
    pub pin21: Option<Peri<'static, embassy_rp::peripherals::PIN_21>>,
    pub pin22: Option<Peri<'static, embassy_rp::peripherals::PIN_22>>,
    pub pin23: Option<Peri<'static, embassy_rp::peripherals::PIN_23>>,
    pub pin24: Option<Peri<'static, embassy_rp::peripherals::PIN_24>>,
    pub pin25: Option<Peri<'static, embassy_rp::peripherals::PIN_25>>,
    pub pin26: Option<Peri<'static, embassy_rp::peripherals::PIN_26>>,
    pub pin27: Option<Peri<'static, embassy_rp::peripherals::PIN_27>>,
    pub pin28: Option<Peri<'static, embassy_rp::peripherals::PIN_28>>,
    pub pin29: Option<Peri<'static, embassy_rp::peripherals::PIN_29>>,
}

impl PinBankPeripherals {
    /// Create from Embassy Peripherals
    pub fn from_peripherals(p: Peripherals) -> (Self, RemainingPeripherals) {
        let pins = Self {
            pin0: Some(p.PIN_0),
            pin1: Some(p.PIN_1),
            pin2: Some(p.PIN_2),
            pin3: Some(p.PIN_3),
            pin4: Some(p.PIN_4),
            pin5: Some(p.PIN_5),
            pin6: Some(p.PIN_6),
            pin7: Some(p.PIN_7),
            pin8: Some(p.PIN_8),
            pin9: Some(p.PIN_9),
            pin10: Some(p.PIN_10),
            pin11: Some(p.PIN_11),
            pin12: Some(p.PIN_12),
            pin13: Some(p.PIN_13),
            pin14: Some(p.PIN_14),
            pin15: Some(p.PIN_15),
            pin16: Some(p.PIN_16),
            pin17: Some(p.PIN_17),
            pin18: Some(p.PIN_18),
            pin19: Some(p.PIN_19),
            pin20: Some(p.PIN_20),
            pin21: Some(p.PIN_21),
            pin22: Some(p.PIN_22),
            pin23: Some(p.PIN_23),
            pin24: Some(p.PIN_24),
            pin25: Some(p.PIN_25),
            pin26: Some(p.PIN_26),
            pin27: Some(p.PIN_27),
            pin28: Some(p.PIN_28),
            pin29: Some(p.PIN_29),
        };
        let remaining = RemainingPeripherals {
            flash: p.FLASH,
            pio0: p.PIO0,
            pio1: p.PIO1,
            uart0: p.UART0,
            uart1: p.UART1,
            adc: p.ADC,
            dma_ch0: p.DMA_CH0,
            dma_ch1: p.DMA_CH1,
            dma_ch2: p.DMA_CH2,
            dma_ch3: p.DMA_CH3,
            // Add more as needed
        };
        (pins, remaining)
    }
}

/// Non-GPIO peripherals that remain after creating PinBank
pub struct RemainingPeripherals {
    pub flash: Peri<'static, embassy_rp::peripherals::FLASH>,
    pub pio0: Peri<'static, embassy_rp::peripherals::PIO0>,
    pub pio1: Peri<'static, embassy_rp::peripherals::PIO1>,
    pub uart0: Peri<'static, embassy_rp::peripherals::UART0>,
    pub uart1: Peri<'static, embassy_rp::peripherals::UART1>,
    pub adc: Peri<'static, embassy_rp::peripherals::ADC>,
    pub dma_ch0: Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    pub dma_ch1: Peri<'static, embassy_rp::peripherals::DMA_CH1>,
    pub dma_ch2: Peri<'static, embassy_rp::peripherals::DMA_CH2>,
    pub dma_ch3: Peri<'static, embassy_rp::peripherals::DMA_CH3>,
}
