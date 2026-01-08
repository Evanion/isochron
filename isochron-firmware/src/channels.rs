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

/// Autotune command types
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AutotuneCommand {
    /// Start autotune with target temperature (°C × 10)
    Start { target_x10: i16 },
    /// Cancel ongoing autotune
    Cancel,
}

/// Autotune status updates
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AutotuneStatus {
    /// Autotune started
    Started,
    /// Progress update (peak count, elapsed ticks)
    Progress { peaks: u8, ticks: u32 },
    /// Autotune completed with coefficients (×100)
    Complete {
        kp_x100: i16,
        ki_x100: i16,
        kd_x100: i16,
    },
    /// Autotune failed
    Failed(AutotuneFailure),
}

/// Autotune failure reasons
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AutotuneFailure {
    /// Over temperature
    OverTemp,
    /// Timeout
    Timeout,
    /// Sensor fault
    SensorFault,
    /// Oscillation too small
    NoOscillation,
    /// User cancelled
    Cancelled,
}

/// Autotune command signal (from controller to heater task)
pub static AUTOTUNE_CMD: Signal<CriticalSectionRawMutex, AutotuneCommand> = Signal::new();

/// Autotune status signal (from heater task to controller)
pub static AUTOTUNE_STATUS: Signal<CriticalSectionRawMutex, AutotuneStatus> = Signal::new();

/// Calibration save request
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CalibrationSaveRequest {
    /// Heater index
    pub heater_index: u8,
    /// Proportional gain (×100)
    pub kp_x100: i16,
    /// Integral gain (×100)
    pub ki_x100: i16,
    /// Derivative gain (×100)
    pub kd_x100: i16,
}

/// Calibration save request signal (from controller to calibration task)
pub static CALIBRATION_SAVE: Signal<CriticalSectionRawMutex, CalibrationSaveRequest> =
    Signal::new();
