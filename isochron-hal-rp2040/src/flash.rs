//! Flash storage driver
//!
//! Uses sequential-storage for wear-leveled key-value storage
//! in the last 64KB of flash.

use embassy_rp::flash::{Async, Flash, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;
use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::cache::NoCache;
use sequential_storage::map;

/// Flash storage configuration
pub const FLASH_SIZE: usize = 2 * 1024 * 1024; // 2MB flash on SKR Pico
pub const CONFIG_PARTITION_SIZE: usize = 64 * 1024; // 64KB for config
pub const CONFIG_PARTITION_START: usize = FLASH_SIZE - CONFIG_PARTITION_SIZE;

/// Flash erase size for RP2040
pub const FLASH_ERASE_SIZE: usize = ERASE_SIZE;

/// Storage keys for configuration data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageKey {
    /// Complete machine configuration (binary postcard format)
    MachineConfig = 0,
    /// Machine configuration as TOML text
    MachineConfigToml = 1,
    /// Reserved for future use
    Reserved2 = 2,
}

impl StorageKey {
    /// Get the key as a byte value
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// Implement the Key trait for sequential-storage
impl sequential_storage::map::Key for StorageKey {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, sequential_storage::map::SerializationError> {
        if buffer.is_empty() {
            return Err(sequential_storage::map::SerializationError::BufferTooSmall);
        }
        buffer[0] = self.as_u8();
        Ok(1)
    }

    fn deserialize_from(buffer: &[u8]) -> Result<(Self, usize), sequential_storage::map::SerializationError> {
        if buffer.is_empty() {
            return Err(sequential_storage::map::SerializationError::BufferTooSmall);
        }
        let key = match buffer[0] {
            0 => StorageKey::MachineConfig,
            1 => StorageKey::MachineConfigToml,
            2 => StorageKey::Reserved2,
            _ => return Err(sequential_storage::map::SerializationError::InvalidFormat),
        };
        Ok((key, 1))
    }
}

/// Flash range for the config partition
pub const CONFIG_RANGE: core::ops::Range<u32> =
    (CONFIG_PARTITION_START as u32)..(FLASH_SIZE as u32);

/// Errors from flash operations
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FlashError {
    /// Flash operation failed
    Flash,
    /// Storage operation failed
    Storage,
    /// Key not found
    NotFound,
    /// Buffer too small
    BufferTooSmall,
    /// Data corrupted
    Corrupted,
}

/// Flash storage interface
///
/// Provides wear-leveled key-value storage for configuration data.
/// Uses sequential-storage for automatic wear leveling.
pub struct FlashStorage<'d> {
    flash: Flash<'d, FLASH, Async, FLASH_SIZE>,
}

impl<'d> FlashStorage<'d> {
    /// Create a new flash storage instance
    pub fn new(flash: FLASH, dma: impl embassy_rp::Peripheral<P = impl embassy_rp::dma::Channel> + 'd) -> Self {
        Self {
            flash: Flash::new(flash, dma),
        }
    }

    /// Read a value by key into the provided buffer
    ///
    /// Returns the number of bytes read, or an error.
    pub async fn read(&mut self, key: StorageKey, buffer: &mut [u8]) -> Result<usize, FlashError> {
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

    /// Write a value by key
    pub async fn write(&mut self, key: StorageKey, data: &[u8]) -> Result<(), FlashError> {
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

    /// Check if a key exists in storage
    pub async fn exists(&mut self, key: StorageKey) -> bool {
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

    /// Erase all stored data
    ///
    /// This erases the entire config partition.
    pub async fn erase_all(&mut self) -> Result<(), FlashError> {
        // Erase the config partition sector by sector
        let start = CONFIG_PARTITION_START as u32;
        let end = FLASH_SIZE as u32;

        self.flash
            .erase(start, end)
            .await
            .map_err(|_| FlashError::Flash)
    }

    /// Get the raw flash peripheral for low-level access
    pub fn flash(&mut self) -> &mut Flash<'d, FLASH, Async, FLASH_SIZE> {
        &mut self.flash
    }
}
