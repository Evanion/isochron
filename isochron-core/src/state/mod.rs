//! State machine for program execution
//!
//! Defines the authoritative runtime behavior of the machine.
//! The state machine is explicit, finite, and deterministic.

pub mod events;
pub mod machine;

pub use events::Event;
pub use machine::{ErrorKind, State};
