//! Flash storage abstractions
//!
//! Provides traits for persistent key-value storage that can be implemented
//! by chip-specific HALs using their flash memory.

/// Storage keys for configuration data
///
/// These keys identify different types of data stored in flash.
/// The actual storage implementation handles wear leveling and
/// data integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum StorageKey {
    /// Complete machine configuration (binary postcard format)
    MachineConfig = 0,
    /// Machine configuration as TOML text
    MachineConfigToml = 1,
    /// PID calibration data for heaters
    PidCalibration = 2,
    /// Reserved for future use
    Reserved3 = 3,
    /// Reserved for future use
    Reserved4 = 4,
}

impl StorageKey {
    /// Get the key as a byte value
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Create a key from a byte value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(StorageKey::MachineConfig),
            1 => Some(StorageKey::MachineConfigToml),
            2 => Some(StorageKey::PidCalibration),
            3 => Some(StorageKey::Reserved3),
            4 => Some(StorageKey::Reserved4),
            _ => None,
        }
    }
}

/// Errors from flash storage operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FlashError {
    /// Flash operation failed
    Flash,
    /// Storage operation failed
    Storage,
    /// Key not found
    NotFound,
    /// Buffer too small for the data
    BufferTooSmall,
    /// Data corrupted or invalid
    Corrupted,
    /// Storage is full
    Full,
}

/// Flash storage trait
///
/// Provides wear-leveled key-value storage for configuration data.
/// Implementations should handle:
/// - Wear leveling across flash sectors
/// - Data integrity (CRC or similar)
/// - Atomic writes where possible
pub trait FlashStorage {
    /// Read a value by key into the provided buffer
    ///
    /// # Arguments
    /// * `key` - The storage key to read
    /// * `buffer` - Buffer to read data into
    ///
    /// # Returns
    /// The number of bytes read, or an error.
    fn read(&mut self, key: StorageKey, buffer: &mut [u8]) -> impl core::future::Future<Output = Result<usize, FlashError>>;

    /// Write a value by key
    ///
    /// # Arguments
    /// * `key` - The storage key to write
    /// * `data` - Data to write
    fn write(&mut self, key: StorageKey, data: &[u8]) -> impl core::future::Future<Output = Result<(), FlashError>>;

    /// Check if a key exists in storage
    fn exists(&mut self, key: StorageKey) -> impl core::future::Future<Output = bool>;

    /// Erase all stored data
    ///
    /// This erases the entire config partition. Use with caution!
    fn erase_all(&mut self) -> impl core::future::Future<Output = Result<(), FlashError>>;
}

// Implement the sequential-storage Key trait when the feature is enabled
#[cfg(feature = "sequential-storage")]
impl sequential_storage::map::Key for StorageKey {
    fn serialize_into(
        &self,
        buffer: &mut [u8],
    ) -> Result<usize, sequential_storage::map::SerializationError> {
        if buffer.is_empty() {
            return Err(sequential_storage::map::SerializationError::BufferTooSmall);
        }
        buffer[0] = self.as_u8();
        Ok(1)
    }

    fn deserialize_from(
        buffer: &[u8],
    ) -> Result<(Self, usize), sequential_storage::map::SerializationError> {
        if buffer.is_empty() {
            return Err(sequential_storage::map::SerializationError::BufferTooSmall);
        }
        match StorageKey::from_u8(buffer[0]) {
            Some(key) => Ok((key, 1)),
            None => Err(sequential_storage::map::SerializationError::InvalidFormat),
        }
    }
}
