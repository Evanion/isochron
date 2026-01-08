//! Events that trigger state transitions

use super::machine::ErrorKind;

/// Events that can trigger state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Event {
    // Lifecycle events
    /// Boot sequence completed successfully
    BootComplete,

    // UI navigation events
    /// User selected a program from the list
    SelectProgram,
    /// User wants to edit a parameter
    EditParameter,
    /// User confirmed parameter edit
    ConfirmEdit,
    /// User wants to go back
    Back,

    // Execution control events
    /// User pressed start
    Start,
    /// User pressed pause
    Pause,
    /// User pressed resume
    Resume,
    /// User pressed abort (long press)
    Abort,
    /// User confirmed action (generic confirmation)
    UserConfirm,

    // Scheduler events
    /// Current profile/step finished
    ProfileFinished,
    /// All steps in program completed
    ProgramFinished,
    /// Move to next step
    NextStep,

    // Spin-off events
    /// Start automated spin-off (has z motor)
    StartSpinOff,
    /// Prompt user to lift basket (manual machine)
    PromptSpinOff,
    /// Spin-off phase completed
    SpinOffFinished,

    // Manual machine events
    /// Prompt user to move to next jar
    PromptNextJar,

    // PID autotune events
    /// User started PID autotune
    StartAutotune,
    /// Autotune completed successfully with new coefficients
    AutotuneComplete,
    /// Autotune failed (timeout, over-temp, sensor fault, etc.)
    AutotuneFailed,
    /// User cancelled autotune
    CancelAutotune,

    // Safety events
    /// Error detected by safety subsystem
    ErrorDetected(ErrorKind),
    /// User acknowledged error
    AcknowledgeError,
}

impl Event {
    /// Check if this event is user-initiated
    pub fn is_user_event(&self) -> bool {
        matches!(
            self,
            Event::SelectProgram
                | Event::EditParameter
                | Event::ConfirmEdit
                | Event::Back
                | Event::Start
                | Event::Pause
                | Event::Resume
                | Event::Abort
                | Event::UserConfirm
                | Event::AcknowledgeError
                | Event::StartAutotune
                | Event::CancelAutotune
        )
    }

    /// Check if this event is from the scheduler
    pub fn is_scheduler_event(&self) -> bool {
        matches!(
            self,
            Event::ProfileFinished
                | Event::ProgramFinished
                | Event::NextStep
                | Event::StartSpinOff
                | Event::PromptSpinOff
                | Event::SpinOffFinished
                | Event::PromptNextJar
        )
    }

    /// Check if this event indicates an error
    pub fn is_error_event(&self) -> bool {
        matches!(self, Event::ErrorDetected(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_events() {
        assert!(Event::Start.is_user_event());
        assert!(Event::Pause.is_user_event());
        assert!(Event::Abort.is_user_event());
        assert!(!Event::ProfileFinished.is_user_event());
        assert!(!Event::ErrorDetected(ErrorKind::MotorStall).is_user_event());
    }

    #[test]
    fn test_scheduler_events() {
        assert!(Event::ProfileFinished.is_scheduler_event());
        assert!(Event::SpinOffFinished.is_scheduler_event());
        assert!(!Event::Start.is_scheduler_event());
    }

    #[test]
    fn test_error_events() {
        assert!(Event::ErrorDetected(ErrorKind::OverTemperature).is_error_event());
        assert!(!Event::Abort.is_error_event());
    }
}
