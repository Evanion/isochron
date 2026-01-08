//! State machine definition
//!
//! All motor, heater, and UI behavior is a function of the current state
//! and an event.

use super::events::Event;

/// Machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum State {
    /// Power-on initialization, hardware checks, config loading
    Boot,
    /// Ready state, program list visible
    Idle,
    /// Program chosen, summary displayed
    ProgramSelected,
    /// User editing program parameters
    EditProgram,
    /// Waiting for user to move basket to jar (manual machines)
    AwaitingJar,
    /// Profile executing (spin motor active in jar)
    Running,
    /// Waiting for user to lift basket for spin-off (manual machines)
    AwaitingSpinOff,
    /// Basket lifted, spinning to shed excess solution
    SpinOff,
    /// Execution paused by user
    Paused,
    /// Current step/jar complete, transitioning to next
    StepComplete,
    /// All steps completed successfully
    ProgramComplete,
    /// PID autotune in progress
    Autotuning,
    /// Fault detected; outputs disabled
    Error(ErrorKind),
}

/// Types of errors that can occur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ErrorKind {
    /// Temperature sensor fault (open/short)
    ThermistorFault,
    /// Temperature exceeded safe limit
    OverTemperature,
    /// Motor stall detected
    MotorStall,
    /// Display communication lost
    LinkLost,
    /// Configuration error
    ConfigError,
    /// Unknown/generic error
    Unknown,
}

impl State {
    /// Check if this state allows motor operation
    pub fn motor_allowed(&self) -> bool {
        matches!(self, State::Running | State::SpinOff)
    }

    /// Check if this state allows heater operation
    pub fn heater_allowed(&self) -> bool {
        // Heater allowed during Running and Autotuning states
        // Not during SpinOff (basket is out of solution)
        matches!(self, State::Running | State::Autotuning)
    }

    /// Check if this is an error state
    pub fn is_error(&self) -> bool {
        matches!(self, State::Error(_))
    }

    /// Check if this is a terminal state requiring user action
    pub fn is_terminal(&self) -> bool {
        matches!(self, State::Idle | State::ProgramComplete | State::Error(_))
    }

    /// Process an event and return the next state
    ///
    /// This is the core state transition logic.
    pub fn transition(self, event: Event) -> Self {
        use Event::*;
        use State::*;

        match (self, event) {
            // Boot transitions
            (Boot, BootComplete) => Idle,
            (Boot, ErrorDetected(kind)) => Error(kind),

            // Idle transitions
            (Idle, SelectProgram) => ProgramSelected,
            (Idle, StartAutotune) => Autotuning,
            (Idle, ErrorDetected(kind)) => Error(kind),

            // ProgramSelected transitions
            (ProgramSelected, EditParameter) => EditProgram,
            (ProgramSelected, Start) => Running,
            (ProgramSelected, Back) => Idle,
            (ProgramSelected, ErrorDetected(kind)) => Error(kind),

            // EditProgram transitions
            (EditProgram, ConfirmEdit) => ProgramSelected,
            (EditProgram, Back) => ProgramSelected,
            (EditProgram, ErrorDetected(kind)) => Error(kind),

            // AwaitingJar transitions (manual machines)
            (AwaitingJar, UserConfirm) => Running,
            (AwaitingJar, Abort) => Idle,
            (AwaitingJar, ErrorDetected(kind)) => Error(kind),

            // Running transitions
            (Running, Pause) => Paused,
            (Running, ProfileFinished) => {
                // Next state depends on whether spin-off is configured
                // This is determined by the scheduler, not the state machine
                // For now, we go to StepComplete; caller handles spin-off
                StepComplete
            }
            (Running, StartSpinOff) => SpinOff,
            (Running, PromptSpinOff) => AwaitingSpinOff, // Manual machines
            (Running, Abort) => Idle,
            (Running, ErrorDetected(kind)) => Error(kind),

            // AwaitingSpinOff transitions (manual machines)
            (AwaitingSpinOff, UserConfirm) => SpinOff,
            (AwaitingSpinOff, Abort) => Idle,
            (AwaitingSpinOff, ErrorDetected(kind)) => Error(kind),

            // SpinOff transitions
            (SpinOff, SpinOffFinished) => StepComplete,
            (SpinOff, Abort) => Idle,
            (SpinOff, ErrorDetected(kind)) => Error(kind),

            // Paused transitions
            (Paused, Resume) => Running,
            (Paused, Abort) => Idle,
            (Paused, ErrorDetected(kind)) => Error(kind),

            // StepComplete transitions
            (StepComplete, NextStep) => Running,
            (StepComplete, PromptNextJar) => AwaitingJar, // Manual machines
            (StepComplete, ProgramFinished) => ProgramComplete,
            (StepComplete, ErrorDetected(kind)) => Error(kind),

            // ProgramComplete transitions
            (ProgramComplete, SelectProgram) => ProgramSelected,
            (ProgramComplete, Back) => Idle,
            (ProgramComplete, ErrorDetected(kind)) => Error(kind),

            // Autotuning transitions
            (Autotuning, AutotuneComplete) => Idle,
            (Autotuning, AutotuneFailed) => Idle,
            (Autotuning, CancelAutotune) => Idle,
            (Autotuning, ErrorDetected(kind)) => Error(kind),

            // Error transitions
            (Error(_), AcknowledgeError) => Idle,

            // Default: stay in current state
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_to_idle() {
        let state = State::Boot;
        let next = state.transition(Event::BootComplete);
        assert_eq!(next, State::Idle);
    }

    #[test]
    fn test_error_from_any_state() {
        let states = [
            State::Idle,
            State::Running,
            State::Paused,
            State::ProgramSelected,
        ];

        for state in states {
            let next = state.transition(Event::ErrorDetected(ErrorKind::OverTemperature));
            assert!(matches!(next, State::Error(ErrorKind::OverTemperature)));
        }
    }

    #[test]
    fn test_abort_returns_to_idle() {
        let states = [
            State::Running,
            State::Paused,
            State::SpinOff,
            State::AwaitingJar,
        ];

        for state in states {
            let next = state.transition(Event::Abort);
            assert_eq!(next, State::Idle);
        }
    }

    #[test]
    fn test_running_flow() {
        let state = State::Running;

        // Pause
        let paused = state.transition(Event::Pause);
        assert_eq!(paused, State::Paused);

        // Resume
        let running = paused.transition(Event::Resume);
        assert_eq!(running, State::Running);

        // Profile finished
        let complete = running.transition(Event::ProfileFinished);
        assert_eq!(complete, State::StepComplete);
    }

    #[test]
    fn test_spinoff_flow() {
        let state = State::Running;

        // Go to spin-off
        let spinoff = state.transition(Event::StartSpinOff);
        assert_eq!(spinoff, State::SpinOff);

        // Spin-off finished
        let complete = spinoff.transition(Event::SpinOffFinished);
        assert_eq!(complete, State::StepComplete);
    }

    #[test]
    fn test_manual_machine_flow() {
        // Manual machine: user prompted to lift basket
        let running = State::Running;
        let awaiting = running.transition(Event::PromptSpinOff);
        assert_eq!(awaiting, State::AwaitingSpinOff);

        // User confirms lift
        let spinoff = awaiting.transition(Event::UserConfirm);
        assert_eq!(spinoff, State::SpinOff);
    }

    #[test]
    fn test_motor_allowed() {
        assert!(State::Running.motor_allowed());
        assert!(State::SpinOff.motor_allowed());
        assert!(!State::Idle.motor_allowed());
        assert!(!State::Paused.motor_allowed());
    }

    #[test]
    fn test_heater_allowed() {
        assert!(State::Running.heater_allowed());
        assert!(State::Autotuning.heater_allowed()); // Heater needed for autotune
        assert!(!State::SpinOff.heater_allowed()); // No heater during spin-off
        assert!(!State::Idle.heater_allowed());
        assert!(!State::Paused.heater_allowed());
    }

    #[test]
    fn test_autotune_flow() {
        // Start autotune from idle
        let idle = State::Idle;
        let autotuning = idle.transition(Event::StartAutotune);
        assert_eq!(autotuning, State::Autotuning);

        // Complete autotune
        let complete = autotuning.transition(Event::AutotuneComplete);
        assert_eq!(complete, State::Idle);

        // Failed autotune returns to idle
        let autotuning = State::Autotuning;
        let failed = autotuning.transition(Event::AutotuneFailed);
        assert_eq!(failed, State::Idle);

        // Cancelled autotune returns to idle
        let autotuning = State::Autotuning;
        let cancelled = autotuning.transition(Event::CancelAutotune);
        assert_eq!(cancelled, State::Idle);

        // Error during autotune
        let autotuning = State::Autotuning;
        let error = autotuning.transition(Event::ErrorDetected(ErrorKind::OverTemperature));
        assert!(matches!(error, State::Error(ErrorKind::OverTemperature)));
    }
}
