//! State machine for program execution
//!
//! Defines the authoritative runtime behavior of the machine.
//! The state machine is explicit, finite, and deterministic.

pub mod machine;
pub mod events;

pub use machine::{State, ErrorKind};
pub use events::Event;
