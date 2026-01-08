//! Flash storage driver for RP2040
//!
//! Uses sequential-storage for wear-leveled key-value storage
//! in the last 64KB of flash.
//!
//! Implements the `FlashStorage` trait from `isochron-hal`.

use embassy_rp::dma::Channel;
use embassy_rp::flash::{Async, Flash, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;
use embassy_rp::Peri;
use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::cache::NoCache;
use sequential_storage::map;

// Re-export shared types from isochron-hal
pub use isochron_hal::flash::{FlashError, StorageKey};

/// Flash storage configuration
pub const FLASH_SIZE: usize = 2 * 1024 * 1024; // 2MB flash on SKR Pico
pub const CONFIG_PARTITION_SIZE: usize = 64 * 1024; // 64KB for config
pub const CONFIG_PARTITION_START: usize = FLASH_SIZE - CONFIG_PARTITION_SIZE;

/// Flash erase size for RP2040
pub const FLASH_ERASE_SIZE: usize = ERASE_SIZE;

/// Flash range for the config partition
pub const CONFIG_RANGE: core::ops::Range<u32> =
    (CONFIG_PARTITION_START as u32)..(FLASH_SIZE as u32);

/// RP2040 Flash storage implementation
///
/// Provides wear-leveled key-value storage for configuration data.
/// Uses sequential-storage for automatic wear leveling.
pub struct Rp2040FlashStorage<'d> {
    flash: Flash<'d, FLASH, Async, FLASH_SIZE>,
}

impl<'d> Rp2040FlashStorage<'d> {
    /// Create a new flash storage instance
    pub fn new(flash: Peri<'d, FLASH>, dma: Peri<'d, impl Channel>) -> Self {
        Self {
            flash: Flash::new(flash, dma),
        }
    }

    /// Get the raw flash peripheral for low-level access
    pub fn flash(&mut self) -> &mut Flash<'d, FLASH, Async, FLASH_SIZE> {
        &mut self.flash
    }
}

// Implement the shared FlashStorage trait
impl<'d> isochron_hal::FlashStorage for Rp2040FlashStorage<'d> {
    async fn read(&mut self, key: StorageKey, buffer: &mut [u8]) -> Result<usize, FlashError> {
        let mut data_buffer = [0u8; 2048]; // Max config size

        let result = map::fetch_item::<StorageKey, &[u8], _>(
            &mut self.flash,
            CONFIG_RANGE,
            &mut NoCache::new(),
            &mut data_buffer,
            &key,
        )
        .await;

        match result {
            Ok(Some(data)) => {
                let len = data.len();
                if buffer.len() < len {
                    return Err(FlashError::BufferTooSmall);
                }
                buffer[..len].copy_from_slice(data);
                Ok(len)
            }
            Ok(None) => Err(FlashError::NotFound),
            Err(_) => Err(FlashError::Storage),
        }
    }

    async fn write(&mut self, key: StorageKey, data: &[u8]) -> Result<(), FlashError> {
        let mut data_buffer = [0u8; 2048];

        map::store_item(
            &mut self.flash,
            CONFIG_RANGE,
            &mut NoCache::new(),
            &mut data_buffer,
            &key,
            &data,
        )
        .await
        .map_err(|_| FlashError::Storage)
    }

    async fn exists(&mut self, key: StorageKey) -> bool {
        let mut data_buffer = [0u8; 2048];

        matches!(
            map::fetch_item::<StorageKey, &[u8], _>(
                &mut self.flash,
                CONFIG_RANGE,
                &mut NoCache::new(),
                &mut data_buffer,
                &key,
            )
            .await,
            Ok(Some(_))
        )
    }

    async fn erase_all(&mut self) -> Result<(), FlashError> {
        // Erase the config partition sector by sector
        let start = CONFIG_PARTITION_START as u32;
        let end = FLASH_SIZE as u32;

        self.flash
            .erase(start, end)
            .await
            .map_err(|_| FlashError::Flash)
    }
}

/// Type alias for backwards compatibility
pub type FlashStorage<'d> = Rp2040FlashStorage<'d>;
