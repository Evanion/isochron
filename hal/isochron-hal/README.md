# isochron-hal

Hardware abstraction traits for the Isochron firmware ecosystem.

This crate defines chip-agnostic traits that are implemented by chip-specific HAL crates (e.g., `isochron-hal-rp2040`, `isochron-hal-stm32f0`).

## Traits

### GPIO

```rust
pub trait OutputPin {
    fn set_high(&mut self);
    fn set_low(&mut self);
    fn toggle(&mut self);
    fn is_set_high(&self) -> bool;
}

pub trait InputPin {
    fn is_high(&self) -> bool;
    fn is_low(&self) -> bool;
}
```

### Communication

```rust
pub trait UartTx {
    async fn write(&mut self, data: &[u8]) -> Result<(), UartError>;
    async fn flush(&mut self) -> Result<(), UartError>;
}

pub trait UartRx {
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, UartError>;
    async fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), UartError>;
}

pub trait I2cBus {
    async fn write(&mut self, address: u8, data: &[u8]) -> Result<(), I2cError>;
    async fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), I2cError>;
    async fn write_read(&mut self, address: u8, write: &[u8], read: &mut [u8]) -> Result<(), I2cError>;
}

pub trait SpiBus {
    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), SpiError>;
    async fn write(&mut self, data: &[u8]) -> Result<(), SpiError>;
}
```

### Flash Storage

```rust
pub trait FlashStorage {
    async fn read(&mut self, key: StorageKey, buffer: &mut [u8]) -> Result<usize, FlashError>;
    async fn write(&mut self, key: StorageKey, data: &[u8]) -> Result<(), FlashError>;
    async fn exists(&mut self, key: StorageKey) -> bool;
    async fn erase_all(&mut self) -> Result<(), FlashError>;
}
```

## Features

- `defmt` - Enable defmt formatting for error types
- `sequential-storage` - Enable `sequential_storage::map::Key` implementation for `StorageKey`

## Implementing a New HAL

To add support for a new chip family:

1. Create a new crate (e.g., `isochron-hal-mychip`)
2. Add `isochron-hal` as a dependency
3. Implement the traits for your chip's peripherals
4. Export the implementations

Example structure:

```rust
// hal/isochron-hal-mychip/src/lib.rs
#![no_std]

pub mod flash;
pub mod gpio;
pub mod uart;

pub use flash::MyChipFlashStorage;
pub use gpio::{MyChipInput, MyChipOutput};
pub use uart::{MyChipUartTx, MyChipUartRx};

// Re-export the trait for convenience
pub use isochron_hal::FlashStorage as FlashStorageTrait;
```

## License

GPL-3.0-or-later
