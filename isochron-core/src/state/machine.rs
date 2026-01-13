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
    /// Homing Z and/or X axes (automated machines)
    Homing,
    /// Ready state, program list visible
    Idle,
    /// Program chosen, summary displayed
    ProgramSelected,
    /// User editing program parameters
    EditProgram,
    /// Waiting for user to move basket to jar (manual machines)
    AwaitingJar,
    /// Profile executing (basket motor active in jar)
    Running,
    /// Waiting for user to lift basket for spin-off (manual machines)
    AwaitingSpinOff,
    /// Basket lifted, spinning to shed excess solution
    SpinOff,
    /// Z axis lifting basket to safe_z (automated machines)
    Lifting,
    /// X axis moving basket to next jar position (fully automated)
    MovingToJar,
    /// Z axis lowering basket into jar (automated machines)
    Lowering,
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
    /// Homing failed (endstop not triggered, timeout)
    HomingFailed,
    /// Position out of bounds
    PositionOutOfBounds,
    /// Unknown/generic error
    Unknown,
}

impl State {
    /// Check if this state allows motor operation
    pub fn motor_allowed(&self) -> bool {
        matches!(
            self,
            State::Running
                | State::SpinOff
                | State::Homing
                | State::Lifting
                | State::MovingToJar
                | State::Lowering
        )
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
            (Boot, StartHoming) => Homing, // Automated machines home on boot
            (Boot, ErrorDetected(kind)) => Error(kind),

            // Homing transitions (automated machines)
            (Homing, HomingComplete) => Idle,
            (Homing, Abort) => Idle,
            (Homing, ErrorDetected(kind)) => Error(kind),

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

            // AwaitingJar transitions (manual and semi-automated machines)
            (AwaitingJar, UserConfirm) => Running, // Manual: user moved jar, start running
            (AwaitingJar, StartLower) => Lowering, // Semi-automated: user confirmed, lower basket
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
            (Running, StartLift) => Lifting,             // Automated machines: lift for spinoff
            (Running, Abort) => Idle,
            (Running, ErrorDetected(kind)) => Error(kind),

            // AwaitingSpinOff transitions (manual machines)
            (AwaitingSpinOff, UserConfirm) => SpinOff,
            (AwaitingSpinOff, Abort) => Idle,
            (AwaitingSpinOff, ErrorDetected(kind)) => Error(kind),

            // SpinOff transitions
            (SpinOff, SpinOffFinished) => StepComplete,
            (SpinOff, StartLift) => Lifting, // Automated: lift after spinoff for jar transition
            (SpinOff, Abort) => Idle,
            (SpinOff, ErrorDetected(kind)) => Error(kind),

            // Paused transitions
            (Paused, Resume) => Running,
            (Paused, Abort) => Idle,
            (Paused, ErrorDetected(kind)) => Error(kind),

            // StepComplete transitions
            (StepComplete, NextStep) => Running,
            (StepComplete, PromptNextJar) => AwaitingJar, // Manual machines
            (StepComplete, StartLift) => Lifting,         // Automated: lift for jar transition
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

            // Lifting transitions (automated machines)
            (Lifting, LiftComplete) => StepComplete, // Scheduler decides next: StartMoveX or PromptNextJar
            (Lifting, StartMoveX) => MovingToJar,    // Fully automated: proceed to X move
            (Lifting, PromptNextJar) => AwaitingJar, // Semi-automated: prompt user to rotate
            (Lifting, Abort) => Idle,
            (Lifting, ErrorDetected(kind)) => Error(kind),

            // MovingToJar transitions (fully automated machines)
            (MovingToJar, MoveXComplete) => StepComplete, // Scheduler decides: StartLower
            (MovingToJar, StartLower) => Lowering,
            (MovingToJar, Abort) => Idle,
            (MovingToJar, ErrorDetected(kind)) => Error(kind),

            // Lowering transitions (automated machines)
            (Lowering, LowerComplete) => Running, // Basket in jar, start profile
            (Lowering, Abort) => Idle,
            (Lowering, ErrorDetected(kind)) => Error(kind),

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
            State::Homing,
            State::Lifting,
            State::MovingToJar,
            State::Lowering,
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
        // Motors allowed during active states
        assert!(State::Running.motor_allowed());
        assert!(State::SpinOff.motor_allowed());
        assert!(State::Homing.motor_allowed());
        assert!(State::Lifting.motor_allowed());
        assert!(State::MovingToJar.motor_allowed());
        assert!(State::Lowering.motor_allowed());

        // Motors not allowed during inactive states
        assert!(!State::Idle.motor_allowed());
        assert!(!State::Paused.motor_allowed());
        assert!(!State::AwaitingJar.motor_allowed());
        assert!(!State::StepComplete.motor_allowed());
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

    #[test]
    fn test_homing_flow() {
        // Automated machine: boot triggers homing
        let boot = State::Boot;
        let homing = boot.transition(Event::StartHoming);
        assert_eq!(homing, State::Homing);

        // Homing complete -> idle
        let idle = homing.transition(Event::HomingComplete);
        assert_eq!(idle, State::Idle);

        // Homing error
        let homing = State::Homing;
        let error = homing.transition(Event::ErrorDetected(ErrorKind::HomingFailed));
        assert!(matches!(error, State::Error(ErrorKind::HomingFailed)));
    }

    #[test]
    fn test_fully_automated_flow() {
        // Full automation: Z and X motors
        // Running -> StepComplete -> Lifting -> MovingToJar -> Lowering -> Running

        // Profile finishes
        let running = State::Running;
        let step_complete = running.transition(Event::ProfileFinished);
        assert_eq!(step_complete, State::StepComplete);

        // Scheduler triggers lift for jar transition
        let lifting = step_complete.transition(Event::StartLift);
        assert_eq!(lifting, State::Lifting);

        // Lift complete, move X
        let moving = lifting.transition(Event::StartMoveX);
        assert_eq!(moving, State::MovingToJar);

        // X move complete, lower basket
        let lowering = moving.transition(Event::StartLower);
        assert_eq!(lowering, State::Lowering);

        // Lower complete, resume running
        let running = lowering.transition(Event::LowerComplete);
        assert_eq!(running, State::Running);
    }

    #[test]
    fn test_semi_automated_flow() {
        // Semi-automation: Z motor only, user rotates carousel
        // Running -> StepComplete -> Lifting -> AwaitingJar -> Lowering -> Running

        // Profile finishes
        let running = State::Running;
        let step_complete = running.transition(Event::ProfileFinished);
        assert_eq!(step_complete, State::StepComplete);

        // Scheduler triggers lift
        let lifting = step_complete.transition(Event::StartLift);
        assert_eq!(lifting, State::Lifting);

        // Lift complete, prompt user to rotate carousel
        let awaiting = lifting.transition(Event::PromptNextJar);
        assert_eq!(awaiting, State::AwaitingJar);

        // User confirms jar position, lower basket
        let lowering = awaiting.transition(Event::StartLower);
        assert_eq!(lowering, State::Lowering);

        // Lower complete, resume running
        let running = lowering.transition(Event::LowerComplete);
        assert_eq!(running, State::Running);
    }

    #[test]
    fn test_automated_spinoff_with_lift() {
        // Automated machine: spinoff with lift
        // Running -> SpinOff -> Lifting -> ...

        let running = State::Running;
        let spinoff = running.transition(Event::StartSpinOff);
        assert_eq!(spinoff, State::SpinOff);

        // After spinoff, lift for jar transition
        let lifting = spinoff.transition(Event::StartLift);
        assert_eq!(lifting, State::Lifting);
    }

    #[test]
    fn test_position_error_kinds() {
        // Test new error kinds
        let lifting = State::Lifting;
        let error = lifting.transition(Event::ErrorDetected(ErrorKind::PositionOutOfBounds));
        assert!(matches!(
            error,
            State::Error(ErrorKind::PositionOutOfBounds)
        ));

        let moving = State::MovingToJar;
        let error = moving.transition(Event::ErrorDetected(ErrorKind::HomingFailed));
        assert!(matches!(error, State::Error(ErrorKind::HomingFailed)));
    }

    #[test]
    fn test_abort_from_position_states() {
        // Test abort from all position-related states
        let states = [
            State::Homing,
            State::Lifting,
            State::MovingToJar,
            State::Lowering,
        ];

        for state in states {
            let result = state.transition(Event::Abort);
            assert_eq!(
                result,
                State::Idle,
                "Abort from {:?} should go to Idle",
                state
            );
        }
    }

    #[test]
    fn test_error_from_position_states() {
        // Test error transitions from all position states
        let states = [
            State::Homing,
            State::Lifting,
            State::MovingToJar,
            State::Lowering,
        ];

        for state in states {
            let result = state.transition(Event::ErrorDetected(ErrorKind::MotorStall));
            assert!(
                matches!(result, State::Error(ErrorKind::MotorStall)),
                "Error from {:?} should go to Error state",
                state
            );
        }
    }

    #[test]
    fn test_lift_complete_transition() {
        // LiftComplete from Lifting goes to StepComplete
        let lifting = State::Lifting;
        let result = lifting.transition(Event::LiftComplete);
        assert_eq!(result, State::StepComplete);
    }

    #[test]
    fn test_move_x_complete_transition() {
        // MoveXComplete from MovingToJar goes to StepComplete
        let moving = State::MovingToJar;
        let result = moving.transition(Event::MoveXComplete);
        assert_eq!(result, State::StepComplete);
    }

    #[test]
    fn test_invalid_transitions_no_change() {
        // Invalid transitions should not change state
        let idle = State::Idle;

        // Can't complete lift when idle
        assert_eq!(idle.transition(Event::LiftComplete), State::Idle);

        // Can't complete move when idle
        assert_eq!(idle.transition(Event::MoveXComplete), State::Idle);

        // Can't lower when idle
        assert_eq!(idle.transition(Event::LowerComplete), State::Idle);

        // Can't pause when idle
        assert_eq!(idle.transition(Event::Pause), State::Idle);
    }

    #[test]
    fn test_awaiting_jar_to_lowering() {
        // Semi-automated: user confirms jar, start lowering
        let awaiting = State::AwaitingJar;
        let lowering = awaiting.transition(Event::StartLower);
        assert_eq!(lowering, State::Lowering);
    }

    #[test]
    fn test_all_error_kinds() {
        // Ensure all error kinds can be transitioned to
        let error_kinds = [
            ErrorKind::ThermistorFault,
            ErrorKind::OverTemperature,
            ErrorKind::MotorStall,
            ErrorKind::LinkLost,
            ErrorKind::ConfigError,
            ErrorKind::HomingFailed,
            ErrorKind::PositionOutOfBounds,
            ErrorKind::Unknown,
        ];

        for kind in error_kinds {
            let running = State::Running;
            let error = running.transition(Event::ErrorDetected(kind));
            assert!(matches!(error, State::Error(k) if k == kind));
        }
    }

    #[test]
    fn test_lifting_to_moving_direct() {
        // Fully automated: can go from Lifting directly to MovingToJar
        let lifting = State::Lifting;
        let moving = lifting.transition(Event::StartMoveX);
        assert_eq!(moving, State::MovingToJar);
    }

    #[test]
    fn test_lifting_to_awaiting() {
        // Semi-automated: can go from Lifting to AwaitingJar
        let lifting = State::Lifting;
        let awaiting = lifting.transition(Event::PromptNextJar);
        assert_eq!(awaiting, State::AwaitingJar);
    }

    #[test]
    fn test_moving_to_lowering_direct() {
        // Can go from MovingToJar directly to Lowering
        let moving = State::MovingToJar;
        let lowering = moving.transition(Event::StartLower);
        assert_eq!(lowering, State::Lowering);
    }

    #[test]
    fn test_position_states_motor_allowed() {
        // All position states should allow motor (for stepper movement)
        assert!(State::Homing.motor_allowed());
        assert!(State::Lifting.motor_allowed());
        assert!(State::MovingToJar.motor_allowed());
        assert!(State::Lowering.motor_allowed());
    }

    #[test]
    fn test_position_states_heater_not_allowed() {
        // Position states should not allow heater (basket not in jar)
        assert!(!State::Homing.heater_allowed());
        assert!(!State::Lifting.heater_allowed());
        assert!(!State::MovingToJar.heater_allowed());
        assert!(!State::Lowering.heater_allowed());
    }
}
