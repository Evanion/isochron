//! Configuration loading and parsing
//!
//! Loads configuration from flash or embedded defaults.
//! Uses TOML format parsed by a custom no_std parser.

pub mod loader;
pub mod toml;

pub use loader::ConfigPersistence;
pub use toml::parse_config;
