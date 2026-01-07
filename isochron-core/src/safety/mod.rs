//! Safety monitoring
//!
//! Detects fault conditions and triggers error states.

pub mod monitor;

pub use monitor::{SafetyMonitor, SafetyStatus};
