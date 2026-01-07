//! Inter-task communication channels
//!
//! Defines the static channels used for communication between Embassy tasks.
//! Uses embassy-sync primitives for safe async communication.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;

use isochron_core::scheduler::{HeaterCommand, MotorCommand};
use isochron_core::state::Event;
use isochron_protocol::InputEvent;

/// Channel capacity for input events from display
const INPUT_CHANNEL_SIZE: usize = 8;

/// Channel capacity for state events
const EVENT_CHANNEL_SIZE: usize = 8;

/// Input events from the V0 Display (encoder rotation, button presses)
pub static INPUT_CHANNEL: Channel<CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_SIZE> =
    Channel::new();

/// State machine events (for logging/debugging)
pub static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, Event, EVENT_CHANNEL_SIZE> =
    Channel::new();

/// Signal that a screen update is ready to be sent
pub static SCREEN_UPDATE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Signal that a heartbeat (PING) was received from display
pub static HEARTBEAT_RECEIVED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Motor command signal (updated by controller)
pub static MOTOR_CMD: Signal<CriticalSectionRawMutex, MotorCommand> = Signal::new();

/// Heater command signal (updated by controller)
pub static HEATER_CMD: Signal<CriticalSectionRawMutex, HeaterCommand> = Signal::new();

/// Temperature reading signal (updated by heater task)
/// Value is temperature in 0.1°C units (e.g., 450 = 45.0°C), or None for sensor fault
pub static TEMP_READING: Signal<CriticalSectionRawMutex, Option<i16>> = Signal::new();

/// Motor stall signal (updated by TMC monitoring task)
/// True if motor stall detected via StallGuard
pub static MOTOR_STALL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
