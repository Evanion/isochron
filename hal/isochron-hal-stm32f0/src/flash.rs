//! Flash storage driver for STM32F0
//!
//! Provides flash storage constants and types for STM32F0 series.
//! STM32F042F6 has 32KB flash with 1KB pages.
//!
//! Note: The actual FlashStorage trait implementation requires embassy-stm32's
//! flash peripheral wrapped with embassy-embedded-hal's BlockingAsync adapter
//! to provide the NorFlash trait required by sequential-storage.

// Re-export shared types from isochron-hal
pub use isochron_hal::flash::{FlashError, StorageKey};

/// Flash storage configuration for STM32F042F6
/// 32KB total flash, we reserve the last 4KB for config storage
#[cfg(feature = "stm32f042f6")]
pub const FLASH_SIZE: usize = 32 * 1024; // 32KB
#[cfg(feature = "stm32f042f6")]
pub const CONFIG_PARTITION_SIZE: usize = 4 * 1024; // 4KB for config (4 pages)

#[cfg(not(feature = "stm32f042f6"))]
pub const FLASH_SIZE: usize = 32 * 1024; // Default
#[cfg(not(feature = "stm32f042f6"))]
pub const CONFIG_PARTITION_SIZE: usize = 4 * 1024;

pub const CONFIG_PARTITION_START: usize = FLASH_SIZE - CONFIG_PARTITION_SIZE;

/// Flash page size for STM32F0 series
pub const FLASH_PAGE_SIZE: usize = 1024; // 1KB pages

/// Flash range for the config partition
pub const CONFIG_RANGE: core::ops::Range<u32> =
    (CONFIG_PARTITION_START as u32)..(FLASH_SIZE as u32);

// Note: FlashStorage implementation for STM32F0 will be added in the display
// firmware crate where the full embassy-stm32 flash types are available.
// The STM32F0 has limited RAM (6KB on F042F6), so storage operations will be
// implemented directly in the firmware rather than through a generic HAL wrapper.
