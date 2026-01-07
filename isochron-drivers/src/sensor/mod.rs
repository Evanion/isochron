//! Temperature sensor implementations

pub mod ntc100k;

pub use ntc100k::{AdcReader, Ntc100kSensor};
