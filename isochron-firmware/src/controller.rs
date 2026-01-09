//! Main controller coordinating state machine, scheduler, and safety
//!
//! The controller is the central brain that:
//! - Processes input events from the display
//! - Updates the state machine
//! - Commands the scheduler
//! - Monitors safety conditions
//! - Generates display updates

use isochron_core::config::{
    JarConfig, MachineCapabilities, ProfileConfig, ProgramConfig, MAX_JARS, MAX_PROFILES,
    MAX_PROGRAMS,
};
use isochron_core::motion::{Axis, PositionError, PositionStatus};
use isochron_core::safety::{SafetyMonitor, SafetyStatus};
use isochron_core::scheduler::{HeaterCommand, MotorCommand, Scheduler};
use isochron_core::state::{ErrorKind, Event, State};
use isochron_protocol::InputEvent;

use heapless::Vec;

/// Special menu item index for autotune (after programs)
const AUTOTUNE_MENU_INDEX: u8 = 254;

/// Default autotune target temperature (°C × 10)
const AUTOTUNE_TARGET_X10: i16 = 450; // 45.0°C

/// Autotune UI phase (sub-state within Autotuning state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutotunePhase {
    /// Showing confirmation screen, waiting for user to confirm
    #[default]
    Confirming,
    /// Autotune is running, showing progress
    Running,
    /// Autotune completed successfully, showing result
    Complete,
    /// Autotune failed, showing error
    Failed,
}

/// Autotune failure reason for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutotuneFailureReason {
    OverTemp,
    Timeout,
    SensorFault,
    NoOscillation,
    Cancelled,
}

impl AutotuneFailureReason {
    /// Get a human-readable description
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OverTemp => "Temperature exceeded limit",
            Self::Timeout => "Autotune timed out",
            Self::SensorFault => "Sensor fault detected",
            Self::NoOscillation => "No oscillation detected",
            Self::Cancelled => "Cancelled by user",
        }
    }
}

/// Homing state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomingState {
    /// Not homing
    #[default]
    Idle,
    /// Homing Z axis
    HomingZ,
    /// Homing X axis (after Z complete)
    HomingX,
}

/// Controller state for coordinating subsystems
pub struct Controller {
    /// Current machine state
    state: State,
    /// Program scheduler
    scheduler: Scheduler,
    /// Safety monitor
    safety: SafetyMonitor,
    /// Machine capabilities
    capabilities: MachineCapabilities,
    /// Available programs
    programs: Vec<ProgramConfig, MAX_PROGRAMS>,
    /// Available profiles (shared)
    profiles: Vec<ProfileConfig, MAX_PROFILES>,
    /// Available jars
    jars: Vec<JarConfig, MAX_JARS>,
    /// Currently selected program index (AUTOTUNE_MENU_INDEX for autotune)
    selected_program: u8,
    /// Last tick timestamp (ms)
    last_tick_ms: u32,
    /// Homing state (for multi-axis homing sequence)
    homing_state: HomingState,
    /// Safe Z position (mm) for automated machines
    safe_z: i32,
    /// Autotune UI phase
    autotune_phase: AutotunePhase,
    /// Autotune progress tracking
    autotune_peaks: u8,
    autotune_elapsed_ticks: u32,
    /// Autotune result (when complete)
    autotune_result: Option<(i16, i16, i16)>,
    /// Autotune failure reason (when failed)
    autotune_failure: Option<AutotuneFailureReason>,
}

impl Controller {
    /// Create a new controller with given capabilities
    pub fn new(capabilities: MachineCapabilities) -> Self {
        Self {
            state: State::Boot,
            scheduler: Scheduler::new(capabilities),
            safety: SafetyMonitor::new(),
            capabilities,
            programs: Vec::new(),
            profiles: Vec::new(),
            jars: Vec::new(),
            selected_program: 0,
            last_tick_ms: 0,
            homing_state: HomingState::default(),
            safe_z: 0,
            autotune_phase: AutotunePhase::default(),
            autotune_peaks: 0,
            autotune_elapsed_ticks: 0,
            autotune_result: None,
            autotune_failure: None,
        }
    }

    /// Create a new controller with capabilities and safe_z position
    pub fn with_safe_z(capabilities: MachineCapabilities, safe_z: i32) -> Self {
        let mut ctrl = Self::new(capabilities);
        ctrl.safe_z = safe_z;
        ctrl
    }

    /// Load configuration data
    pub fn load_config(
        &mut self,
        programs: &[ProgramConfig],
        profiles: &[ProfileConfig],
        jars: &[JarConfig],
    ) {
        self.programs.clear();
        self.profiles.clear();
        self.jars.clear();

        for p in programs.iter().take(MAX_PROGRAMS) {
            let _ = self.programs.push(p.clone());
        }
        for p in profiles.iter().take(MAX_PROFILES) {
            let _ = self.profiles.push(p.clone());
        }
        for j in jars.iter().take(MAX_JARS) {
            let _ = self.jars.push(j.clone());
        }

        // Also load into scheduler
        self.scheduler.load_profiles(profiles);
        self.scheduler.load_jars(jars);
    }

    /// Complete boot sequence
    ///
    /// For manual machines, transitions directly to Idle.
    /// For automated machines (with Z or X motors), returns StartHoming event.
    pub fn boot_complete(&mut self) -> Option<Event> {
        if self.capabilities.has_z || self.capabilities.has_x {
            // Automated machine - need to home first
            self.transition(Event::StartHoming);
            Some(Event::StartHoming)
        } else {
            // Manual machine - go directly to idle
            self.transition(Event::BootComplete);
            None
        }
    }

    /// Check if homing is needed (called during boot)
    pub fn needs_homing(&self) -> bool {
        self.capabilities.has_z || self.capabilities.has_x
    }

    /// Start the homing sequence
    ///
    /// Returns the first axis to home (Z first if available, then X).
    /// Returns None if no axes need homing.
    pub fn start_homing(&mut self) -> Option<Axis> {
        if self.capabilities.has_z {
            self.homing_state = HomingState::HomingZ;
            Some(Axis::Z)
        } else if self.capabilities.has_x {
            self.homing_state = HomingState::HomingX;
            Some(Axis::X)
        } else {
            self.homing_state = HomingState::Idle;
            None
        }
    }

    /// Handle position status event from position tasks
    ///
    /// Returns an event if state should change.
    pub fn handle_position_status(&mut self, status: PositionStatus) -> Option<Event> {
        match status {
            PositionStatus::Homed(axis) => {
                match (axis, self.homing_state) {
                    (Axis::Z, HomingState::HomingZ) => {
                        // Z homing complete
                        if self.capabilities.has_x {
                            // Now home X
                            self.homing_state = HomingState::HomingX;
                            // Return None - caller will send HomeX command
                            None
                        } else {
                            // All homing complete
                            self.homing_state = HomingState::Idle;
                            self.transition(Event::HomingComplete);
                            Some(Event::HomingComplete)
                        }
                    }
                    (Axis::X, HomingState::HomingX) => {
                        // X homing complete - all done
                        self.homing_state = HomingState::Idle;
                        self.transition(Event::HomingComplete);
                        Some(Event::HomingComplete)
                    }
                    _ => None,
                }
            }
            PositionStatus::Complete(axis) => {
                // Position move completed
                match self.state {
                    State::Lifting if axis == Axis::Z => {
                        Some(Event::LiftComplete)
                    }
                    State::MovingToJar if axis == Axis::X => {
                        Some(Event::MoveXComplete)
                    }
                    State::Lowering if axis == Axis::Z => {
                        self.transition(Event::LowerComplete);
                        Some(Event::LowerComplete)
                    }
                    _ => None,
                }
            }
            PositionStatus::Error { axis: _, kind } => {
                // Convert position error to state machine error
                let error_kind = match kind {
                    PositionError::EndstopNotTriggered | PositionError::Timeout => {
                        ErrorKind::HomingFailed
                    }
                    PositionError::OutOfBounds => ErrorKind::PositionOutOfBounds,
                    PositionError::NotHomed => ErrorKind::HomingFailed,
                    PositionError::StallDetected => ErrorKind::MotorStall,
                };
                self.homing_state = HomingState::Idle;
                self.transition(Event::ErrorDetected(error_kind));
                Some(Event::ErrorDetected(error_kind))
            }
        }
    }

    /// Get current homing state
    pub fn homing_state(&self) -> HomingState {
        self.homing_state
    }

    /// Get machine capabilities
    pub fn capabilities(&self) -> &MachineCapabilities {
        &self.capabilities
    }

    /// Get safe Z position
    pub fn safe_z(&self) -> i32 {
        self.safe_z
    }

    // --- Jar transition methods ---

    /// Start lift sequence (called when step completes and needs jar transition)
    ///
    /// Returns the target Z position (safe_z) if lift should start.
    pub fn start_lift(&mut self) -> Option<i32> {
        // Can start lift from Running, SpinOff, or StepComplete states
        let can_lift = matches!(
            self.state,
            State::Running | State::SpinOff | State::StepComplete
        );

        if self.capabilities.has_z && can_lift {
            self.transition(Event::StartLift);
            Some(self.safe_z)
        } else {
            None
        }
    }

    /// Check if lift is complete and determine next action
    ///
    /// Returns:
    /// - Some((Event::StartMoveX, x_pos)) for fully automated (has_x)
    /// - Some((Event::PromptNextJar, 0)) for semi-automated (Z only)
    /// - None if not in Lifting state or no next jar
    pub fn handle_lift_complete(&mut self) -> Option<(Event, i32)> {
        if self.state != State::Lifting {
            return None;
        }

        // Get next jar position - copy values to avoid borrow conflict
        let x_pos = self.scheduler.current_jar()?.x_pos;

        if self.capabilities.has_x {
            // Fully automated - move X to jar position
            self.transition(Event::StartMoveX);
            Some((Event::StartMoveX, x_pos))
        } else {
            // Semi-automated - prompt user to rotate carousel
            self.transition(Event::PromptNextJar);
            Some((Event::PromptNextJar, 0))
        }
    }

    /// Check if X move is complete and start lowering
    ///
    /// Returns the target Z position (jar z_pos) if lowering should start.
    pub fn handle_move_x_complete(&mut self) -> Option<i32> {
        if self.state != State::MovingToJar {
            return None;
        }

        // Copy z_pos to avoid borrow conflict
        let z_pos = self.scheduler.current_jar()?.z_pos;
        self.transition(Event::StartLower);
        Some(z_pos)
    }

    /// Handle user confirmation to proceed after semi-automated jar change
    ///
    /// Returns the target Z position (jar z_pos) if lowering should start.
    pub fn handle_jar_confirmed(&mut self) -> Option<i32> {
        if self.state != State::AwaitingJar {
            return None;
        }

        // Copy z_pos to avoid borrow conflict
        let z_pos = self.scheduler.current_jar()?.z_pos;
        self.transition(Event::StartLower);
        Some(z_pos)
    }

    /// Check if lowering is complete and resume running
    ///
    /// This also advances to the next step since the lift/move/lower sequence
    /// replaced the normal advance_step flow for automated machines.
    pub fn handle_lower_complete(&mut self) {
        if self.state == State::Lowering {
            // Advance to next step now that we're in the new jar
            let _ = self.scheduler.advance_step();
            self.transition(Event::LowerComplete);
        }
    }

    /// Get current state
    pub fn state(&self) -> State {
        self.state
    }

    /// Get current motor command
    pub fn motor_command(&self) -> MotorCommand {
        self.scheduler.motor_command()
    }

    /// Get current heater command
    pub fn heater_command(&self) -> HeaterCommand {
        self.scheduler.heater_command()
    }

    /// Get selected program index
    pub fn selected_program(&self) -> u8 {
        self.selected_program
    }

    /// Get program by index
    pub fn get_program(&self, index: u8) -> Option<&ProgramConfig> {
        self.programs.get(index as usize)
    }

    /// Get current profile (if running)
    pub fn current_profile(&self) -> Option<&ProfileConfig> {
        self.scheduler.current_profile()
    }

    /// Get current jar (if running)
    pub fn current_jar(&self) -> Option<&JarConfig> {
        self.scheduler.current_jar()
    }

    /// Get program list for display
    pub fn program_labels(&self) -> impl Iterator<Item = &str> {
        self.programs.iter().map(|p| p.label.as_str())
    }

    /// Process an input event from the display
    pub fn process_input(&mut self, input: InputEvent) -> Option<Event> {
        match input {
            InputEvent::EncoderCw => self.handle_encoder_cw(),
            InputEvent::EncoderCcw => self.handle_encoder_ccw(),
            InputEvent::EncoderClick => self.handle_button_click(),
            InputEvent::EncoderLongPress => self.handle_button_long_press(),
            InputEvent::EncoderRelease => None,
        }
    }

    /// Handle encoder clockwise rotation
    fn handle_encoder_cw(&mut self) -> Option<Event> {
        match self.state {
            State::Idle => {
                // Navigate program list (programs + autotune item)
                if self.selected_program == AUTOTUNE_MENU_INDEX {
                    // Wrap from autotune to first program
                    self.selected_program = 0;
                } else if self.selected_program >= self.programs.len() as u8 - 1 {
                    // Move to autotune
                    self.selected_program = AUTOTUNE_MENU_INDEX;
                } else {
                    self.selected_program += 1;
                }
                None
            }
            State::ProgramSelected | State::EditProgram => {
                // Could adjust parameters here
                None
            }
            _ => None,
        }
    }

    /// Handle encoder counter-clockwise rotation
    fn handle_encoder_ccw(&mut self) -> Option<Event> {
        match self.state {
            State::Idle => {
                // Navigate program list backwards (programs + autotune item)
                if self.selected_program == AUTOTUNE_MENU_INDEX {
                    // Move from autotune to last program
                    if !self.programs.is_empty() {
                        self.selected_program = (self.programs.len() - 1) as u8;
                    }
                } else if self.selected_program == 0 {
                    // Wrap to autotune
                    self.selected_program = AUTOTUNE_MENU_INDEX;
                } else {
                    self.selected_program -= 1;
                }
                None
            }
            State::ProgramSelected | State::EditProgram => {
                // Could adjust parameters here
                None
            }
            _ => None,
        }
    }

    /// Handle button click
    fn handle_button_click(&mut self) -> Option<Event> {
        match self.state {
            State::Idle => {
                if self.selected_program == AUTOTUNE_MENU_INDEX {
                    // Show autotune confirmation screen
                    self.autotune_phase = AutotunePhase::Confirming;
                    self.autotune_peaks = 0;
                    self.autotune_elapsed_ticks = 0;
                    self.autotune_result = None;
                    self.autotune_failure = None;
                    self.transition(Event::StartAutotune);
                    // Don't emit StartAutotune event yet - just show confirm screen
                    None
                } else {
                    // Select program
                    self.transition(Event::SelectProgram);
                    Some(Event::SelectProgram)
                }
            }
            State::ProgramSelected => {
                // Start program
                self.start_program()
            }
            State::AwaitingJar | State::AwaitingSpinOff => {
                // User confirms basket position
                self.scheduler.user_confirm();
                self.transition(Event::UserConfirm);
                Some(Event::UserConfirm)
            }
            State::Running => {
                // Pause
                self.scheduler.pause();
                self.transition(Event::Pause);
                Some(Event::Pause)
            }
            State::Paused => {
                // Resume
                self.scheduler.resume();
                self.transition(Event::Resume);
                Some(Event::Resume)
            }
            State::StepComplete => {
                // For automated machines with Z, need to lift before moving to next jar
                // The lift sequence will eventually call advance_step after lowering
                if self.capabilities.has_z && self.scheduler.next_jar_differs() {
                    // Need to change jars - start lift sequence
                    self.transition(Event::StartLift);
                    Some(Event::StartLift)
                } else {
                    // Same jar or no Z - advance directly
                    if let Some(event) = self.scheduler.advance_step() {
                        self.transition(event);
                        Some(event)
                    } else {
                        None
                    }
                }
            }
            State::ProgramComplete => {
                // Back to idle
                self.transition(Event::Back);
                Some(Event::Back)
            }
            State::Error(_) => {
                // Acknowledge error
                self.transition(Event::AcknowledgeError);
                Some(Event::AcknowledgeError)
            }
            State::Autotuning => {
                match self.autotune_phase {
                    AutotunePhase::Confirming => {
                        // User confirmed - actually start autotune
                        self.autotune_phase = AutotunePhase::Running;
                        Some(Event::StartAutotune)
                    }
                    AutotunePhase::Running => {
                        // Still in progress - ignore click
                        None
                    }
                    AutotunePhase::Complete => {
                        // Dismiss result and go back to idle
                        self.autotune_result = None;
                        self.autotune_phase = AutotunePhase::Confirming;
                        self.transition(Event::AutotuneComplete);
                        Some(Event::AutotuneComplete)
                    }
                    AutotunePhase::Failed => {
                        // Dismiss failure and go back to idle
                        self.autotune_failure = None;
                        self.autotune_phase = AutotunePhase::Confirming;
                        self.transition(Event::AutotuneFailed);
                        Some(Event::AutotuneFailed)
                    }
                }
            }
            _ => None,
        }
    }

    /// Handle button long press
    fn handle_button_long_press(&mut self) -> Option<Event> {
        match self.state {
            State::Running | State::Paused | State::SpinOff => {
                // Abort
                self.scheduler.abort();
                self.transition(Event::Abort);
                Some(Event::Abort)
            }
            State::ProgramSelected => {
                // Back to idle
                self.transition(Event::Back);
                Some(Event::Back)
            }
            State::Autotuning => {
                match self.autotune_phase {
                    AutotunePhase::Confirming => {
                        // Go back to idle without starting
                        self.autotune_phase = AutotunePhase::Confirming;
                        self.transition(Event::CancelAutotune);
                        None // No event - didn't actually start
                    }
                    AutotunePhase::Running => {
                        // Cancel running autotune
                        self.autotune_failure = Some(AutotuneFailureReason::Cancelled);
                        self.autotune_phase = AutotunePhase::Failed;
                        Some(Event::CancelAutotune)
                    }
                    AutotunePhase::Complete | AutotunePhase::Failed => {
                        // Already done - just dismiss
                        self.autotune_result = None;
                        self.autotune_failure = None;
                        self.autotune_phase = AutotunePhase::Confirming;
                        self.transition(Event::CancelAutotune);
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// Start the currently selected program
    fn start_program(&mut self) -> Option<Event> {
        if let Some(program) = self.programs.get(self.selected_program as usize) {
            if let Some(event) = self.scheduler.start_program(program.clone()) {
                self.transition(event);
                return Some(event);
            }
            // Program started successfully
            self.transition(Event::Start);
            Some(Event::Start)
        } else {
            None
        }
    }

    /// Update safety with temperature reading
    pub fn update_temperature(&mut self, temp_x10: Option<i16>) {
        self.safety.update_temperature(temp_x10);
    }

    /// Update safety with motor stall status
    pub fn update_motor_stall(&mut self, stalled: bool) {
        self.safety.update_motor_stall(stalled);
    }

    /// Record heartbeat from display
    pub fn heartbeat_received(&mut self) {
        self.safety.heartbeat_received();
    }

    /// Periodic tick update
    ///
    /// Call this regularly (e.g., every 100ms) with the current timestamp.
    /// Returns an event if state should change.
    pub fn tick(&mut self, now_ms: u32) -> Option<Event> {
        let delta_ms = now_ms.wrapping_sub(self.last_tick_ms);
        self.last_tick_ms = now_ms;

        // Update safety monitor time tracking
        self.safety.update_time(delta_ms);

        // Check safety conditions
        if let SafetyStatus::Fault(kind) = self.safety.check() {
            // Only transition to error if not already in error state
            if !self.state.is_error() {
                self.scheduler.abort();
                self.transition(Event::ErrorDetected(kind));
                return Some(Event::ErrorDetected(kind));
            }
        }

        // Update scheduler (only if in running states)
        if self.state.motor_allowed() {
            // Convert delta to seconds for scheduler (rough, accumulates error)
            let delta_s = (delta_ms / 1000) as u16;
            if delta_s > 0 {
                if let Some(event) = self.scheduler.tick(delta_s) {
                    self.transition(event);
                    return Some(event);
                }
            }
        }

        None
    }

    /// Perform state transition
    fn transition(&mut self, event: Event) {
        self.state = self.state.transition(event);
    }

    /// Get elapsed time in current step (seconds)
    pub fn step_elapsed_s(&self) -> u32 {
        self.scheduler
            .step_state()
            .map(|s| s.step_elapsed_s)
            .unwrap_or(0)
    }

    /// Get total time for current step (seconds)
    pub fn step_total_s(&self) -> u32 {
        self.scheduler.step_total_s()
    }

    /// Get current step number (1-indexed)
    pub fn current_step_num(&self) -> u8 {
        self.scheduler
            .step_state()
            .map(|s| s.step_index + 1)
            .unwrap_or(0)
    }

    /// Get total steps in current program
    pub fn total_steps(&self) -> u8 {
        self.scheduler
            .step_state()
            .map(|s| s.total_steps)
            .unwrap_or(0)
    }

    /// Get current temperature in whole degrees (if available)
    pub fn current_temp_c(&self) -> Option<i16> {
        self.safety.get_temperature()
    }

    // === Autotune methods ===

    /// Check if autotune is selected in the menu
    pub fn is_autotune_selected(&self) -> bool {
        self.selected_program == AUTOTUNE_MENU_INDEX
    }

    /// Get the current autotune phase
    pub fn autotune_phase(&self) -> AutotunePhase {
        self.autotune_phase
    }

    /// Get the autotune target temperature in °C
    pub fn autotune_target_c(&self) -> i16 {
        AUTOTUNE_TARGET_X10 / 10
    }

    /// Get the autotune target temperature in °C × 10
    pub fn autotune_target_x10(&self) -> i16 {
        AUTOTUNE_TARGET_X10
    }

    /// Update autotune progress from heater task
    pub fn update_autotune_progress(&mut self, peaks: u8, ticks: u32) {
        self.autotune_peaks = peaks;
        self.autotune_elapsed_ticks = ticks;
    }

    /// Set autotune result when complete (from heater task)
    pub fn set_autotune_complete(&mut self, kp_x100: i16, ki_x100: i16, kd_x100: i16) {
        self.autotune_result = Some((kp_x100, ki_x100, kd_x100));
        self.autotune_phase = AutotunePhase::Complete;
    }

    /// Set autotune failure (from heater task)
    pub fn set_autotune_failed(&mut self, reason: AutotuneFailureReason) {
        self.autotune_failure = Some(reason);
        self.autotune_phase = AutotunePhase::Failed;
    }

    /// Get autotune progress (peaks, elapsed ticks)
    pub fn autotune_progress(&self) -> (u8, u32) {
        (self.autotune_peaks, self.autotune_elapsed_ticks)
    }

    /// Get autotune result if available
    pub fn autotune_result(&self) -> Option<(i16, i16, i16)> {
        self.autotune_result
    }

    /// Get autotune failure reason if available
    pub fn autotune_failure(&self) -> Option<AutotuneFailureReason> {
        self.autotune_failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heapless::String;
    use isochron_core::scheduler::DirectionMode;

    fn make_profile(name: &str, rpm: u16, time_s: u16) -> ProfileConfig {
        let mut label = String::new();
        let _ = label.push_str(name);
        ProfileConfig {
            label,
            rpm,
            time_s,
            direction: DirectionMode::Clockwise,
            iterations: 1,
            ..Default::default()
        }
    }

    fn make_jar(name: &str) -> JarConfig {
        let mut jar_name = String::new();
        let _ = jar_name.push_str(name);
        JarConfig {
            name: jar_name,
            ..Default::default()
        }
    }

    fn make_program(name: &str, steps: &[(&str, &str)]) -> ProgramConfig {
        use isochron_core::config::ProgramStep;
        let mut label = String::new();
        let _ = label.push_str(name);
        let mut step_vec = heapless::Vec::new();
        for (jar, profile) in steps {
            let mut j = String::new();
            let _ = j.push_str(jar);
            let mut p = String::new();
            let _ = p.push_str(profile);
            let _ = step_vec.push(ProgramStep { jar: j, profile: p });
        }
        ProgramConfig {
            label,
            steps: step_vec,
        }
    }

    #[test]
    fn test_controller_boot() {
        let mut ctrl = Controller::new(MachineCapabilities::default());
        assert_eq!(ctrl.state(), State::Boot);

        ctrl.boot_complete();
        assert_eq!(ctrl.state(), State::Idle);
    }

    #[test]
    fn test_program_selection() {
        let mut ctrl = Controller::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 10)];
        let jars = [make_jar("clean")];
        let programs = [make_program("Test", &[("clean", "Clean")])];

        ctrl.load_config(&programs, &profiles, &jars);
        ctrl.boot_complete();

        // Select and start program
        ctrl.process_input(InputEvent::EncoderClick); // Select
        assert_eq!(ctrl.state(), State::ProgramSelected);

        ctrl.process_input(InputEvent::EncoderClick); // Start
        assert_eq!(ctrl.state(), State::Running);
    }

    #[test]
    fn test_pause_resume() {
        let mut ctrl = Controller::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 60)];
        let jars = [make_jar("clean")];
        let programs = [make_program("Test", &[("clean", "Clean")])];

        ctrl.load_config(&programs, &profiles, &jars);
        ctrl.boot_complete();
        ctrl.process_input(InputEvent::EncoderClick); // Select
        ctrl.process_input(InputEvent::EncoderClick); // Start

        // Pause
        ctrl.process_input(InputEvent::EncoderClick);
        assert_eq!(ctrl.state(), State::Paused);
        assert_eq!(ctrl.motor_command(), MotorCommand::stopped());

        // Resume
        ctrl.process_input(InputEvent::EncoderClick);
        assert_eq!(ctrl.state(), State::Running);
        assert_eq!(ctrl.motor_command().rpm, 120);
    }

    #[test]
    fn test_safety_override() {
        let mut ctrl = Controller::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 60)];
        let jars = [make_jar("clean")];
        let programs = [make_program("Test", &[("clean", "Clean")])];

        ctrl.load_config(&programs, &profiles, &jars);
        ctrl.boot_complete();
        ctrl.process_input(InputEvent::EncoderClick); // Select
        ctrl.process_input(InputEvent::EncoderClick); // Start

        assert_eq!(ctrl.state(), State::Running);

        // Simulate over-temperature
        ctrl.update_temperature(Some(560)); // 56°C > 55°C max

        // Tick should detect the fault
        let event = ctrl.tick(100);
        assert_eq!(
            event,
            Some(Event::ErrorDetected(ErrorKind::OverTemperature))
        );
        assert!(matches!(
            ctrl.state(),
            State::Error(ErrorKind::OverTemperature)
        ));
    }

    #[test]
    fn test_encoder_navigation() {
        let mut ctrl = Controller::new(MachineCapabilities::default());

        let profiles = [
            make_profile("Clean", 120, 60),
            make_profile("Rinse", 100, 30),
        ];
        let jars = [make_jar("clean"), make_jar("rinse")];
        let programs = [
            make_program("Full", &[("clean", "Clean"), ("rinse", "Rinse")]),
            make_program("Quick", &[("clean", "Clean")]),
        ];

        ctrl.load_config(&programs, &profiles, &jars);
        ctrl.boot_complete();

        assert_eq!(ctrl.selected_program(), 0);

        // Navigate CW
        ctrl.process_input(InputEvent::EncoderCw);
        assert_eq!(ctrl.selected_program(), 1);

        // Navigate CW wraps around
        ctrl.process_input(InputEvent::EncoderCw);
        assert_eq!(ctrl.selected_program(), 0);

        // Navigate CCW
        ctrl.process_input(InputEvent::EncoderCcw);
        assert_eq!(ctrl.selected_program(), 1);
    }

    #[test]
    fn test_long_press_abort() {
        let mut ctrl = Controller::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 60)];
        let jars = [make_jar("clean")];
        let programs = [make_program("Test", &[("clean", "Clean")])];

        ctrl.load_config(&programs, &profiles, &jars);
        ctrl.boot_complete();
        ctrl.process_input(InputEvent::EncoderClick); // Select
        ctrl.process_input(InputEvent::EncoderClick); // Start

        assert_eq!(ctrl.state(), State::Running);

        // Long press to abort
        ctrl.process_input(InputEvent::EncoderLongPress);
        assert_eq!(ctrl.state(), State::Idle);
    }
}
