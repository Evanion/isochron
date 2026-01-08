//! Configuration persistence
//!
//! Loads machine configuration from flash storage.
//! Falls back to embedded defaults if flash is empty.

extern crate alloc;

use core::str;
use defmt::*;

use isochron_core::config::MachineConfig;
use isochron_hal_rp2040::flash::{FlashError, FlashStorage, StorageKey};
// Import the FlashStorage trait to bring methods into scope
use isochron_hal_rp2040::FlashStorageTrait;

use super::toml::parse_config;

/// Maximum serialized config size (binary)
const MAX_CONFIG_SIZE: usize = 2048;

/// Maximum TOML config size
const MAX_TOML_SIZE: usize = 8192;

/// Configuration persistence errors
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigError {
    /// Flash operation failed
    Flash(FlashError),
    /// Deserialization failed
    Deserialize,
    /// TOML parsing failed
    TomlParse,
    /// Invalid UTF-8 in TOML data
    InvalidUtf8,
    /// Config version mismatch
    VersionMismatch,
}

impl From<FlashError> for ConfigError {
    fn from(e: FlashError) -> Self {
        ConfigError::Flash(e)
    }
}

/// Configuration persistence manager
///
/// Handles loading machine configuration from flash storage.
pub struct ConfigPersistence<'d> {
    storage: FlashStorage<'d>,
}

impl<'d> ConfigPersistence<'d> {
    /// Create a new config persistence manager
    pub fn new(storage: FlashStorage<'d>) -> Self {
        Self { storage }
    }

    /// Consume this persistence manager and return the underlying storage
    ///
    /// Use this to reclaim the FlashStorage after loading config, so it can
    /// be passed to other tasks (e.g., calibration persistence).
    pub fn into_storage(self) -> FlashStorage<'d> {
        self.storage
    }

    /// Load configuration from flash
    ///
    /// Tries to load TOML config first, falls back to binary postcard format.
    /// Returns the loaded config, or an error if not found or invalid.
    pub async fn load(&mut self) -> Result<MachineConfig, ConfigError> {
        info!("Loading configuration from flash...");

        // Try TOML first
        match self.load_toml().await {
            Ok(config) => {
                info!("Loaded configuration from TOML");
                return Ok(config);
            }
            Err(ConfigError::Flash(FlashError::NotFound)) => {
                debug!("No TOML config found, trying binary format");
            }
            Err(e) => {
                warn!("Failed to load TOML config: {:?}, trying binary", e);
            }
        }

        // Fall back to binary postcard format
        self.load_binary().await
    }

    /// Load configuration from TOML format
    async fn load_toml(&mut self) -> Result<MachineConfig, ConfigError> {
        // Read raw TOML data from flash
        let mut buffer = [0u8; MAX_TOML_SIZE];
        let len = self
            .storage
            .read(StorageKey::MachineConfigToml, &mut buffer)
            .await?;

        debug!("Read {} bytes of TOML from flash", len);

        // Convert to string
        let toml_str = str::from_utf8(&buffer[..len]).map_err(|_| ConfigError::InvalidUtf8)?;

        // Parse TOML
        let config = parse_config(toml_str).map_err(|e| {
            warn!("TOML parse error: {:?}", defmt::Debug2Format(&e));
            ConfigError::TomlParse
        })?;

        log_config_summary(&config);
        Ok(config)
    }

    /// Load configuration from binary postcard format
    async fn load_binary(&mut self) -> Result<MachineConfig, ConfigError> {
        // Read raw data from flash
        let mut buffer = [0u8; MAX_CONFIG_SIZE];
        let len = self
            .storage
            .read(StorageKey::MachineConfig, &mut buffer)
            .await?;

        debug!("Read {} bytes of binary config from flash", len);

        // Deserialize with postcard
        let config: MachineConfig =
            postcard::from_bytes(&buffer[..len]).map_err(|_| ConfigError::Deserialize)?;

        // Version check
        if config.version != 1 {
            warn!(
                "Config version mismatch: found {}, expected 1",
                config.version
            );
            return Err(ConfigError::VersionMismatch);
        }

        log_config_summary(&config);
        Ok(config)
    }
}

/// Log a summary of the loaded configuration
fn log_config_summary(config: &MachineConfig) {
    info!("Configuration loaded successfully");
    debug!("  {} steppers", config.steppers.len());
    debug!("  {} TMC2209 drivers", config.tmc2209s.len());
    debug!("  {} heaters", config.heaters.len());
    debug!("  {} jars", config.jars.len());
    debug!("  {} profiles", config.profiles.len());
    debug!("  {} programs", config.programs.len());
}
