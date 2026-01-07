//! Configuration types
//!
//! Board-agnostic configuration structures stored as postcard binary data.

pub mod hardware;
pub mod types;

pub use hardware::*;
pub use types::*;
