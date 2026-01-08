//! Isochron Hardware Abstraction Layer
//!
//! This crate defines hardware abstraction traits that can be implemented
//! by chip-specific HALs (RP2040, STM32F0, etc.). This enables the same
//! application code to run on different hardware platforms.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Application (isochron-firmware, etc.)  │
//! └─────────────────────────────────────────┘
//!                     │
//!                     ▼
//! ┌─────────────────────────────────────────┐
//! │  isochron-hal (this crate - traits)     │
//! └─────────────────────────────────────────┘
//!                     │
//!         ┌───────────┴───────────┐
//!         ▼                       ▼
//! ┌───────────────┐       ┌───────────────┐
//! │ isochron-hal- │       │ isochron-hal- │
//! │    rp2040     │       │   stm32f0     │
//! └───────────────┘       └───────────────┘
//! ```
//!
//! # Traits
//!
//! - [`gpio::OutputPin`], [`gpio::InputPin`] - Digital I/O
//! - [`uart::UartTx`], [`uart::UartRx`] - Serial communication
//! - [`i2c::I2cBus`] - I2C bus operations
//! - [`spi::SpiBus`] - SPI bus operations
//! - [`flash::FlashStorage`] - Persistent storage

#![no_std]
#![deny(unsafe_code)]

pub mod flash;
pub mod gpio;
pub mod i2c;
pub mod spi;
pub mod uart;

// Re-export key traits at crate root for convenience
pub use flash::{FlashStorage, StorageKey};
pub use gpio::{InputPin, OutputPin};
pub use i2c::I2cBus;
pub use spi::SpiBus;
pub use uart::{UartRx, UartTx};
