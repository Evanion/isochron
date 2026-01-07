//! Program execution scheduler
//!
//! Manages the execution of cleaning programs, tracking current step,
//! segment, and elapsed time. Generates events for state machine transitions.

use heapless::Vec;

use super::segment::{generate_segments, Segment, SpinOffConfig};
use crate::config::{
    JarConfig, MachineCapabilities, ProfileConfig, ProgramConfig, MAX_JARS, MAX_PROFILES,
};
use crate::state::events::Event;
use crate::traits::Direction;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Maximum segments per profile execution
pub const MAX_SEGMENTS: usize = 16;

/// Scheduler execution phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExecutionPhase {
    /// Not running
    Idle,
    /// Executing main profile (motor spinning in jar)
    Running,
    /// Spin-off phase (shedding excess solution)
    SpinOff,
    /// Waiting for user to lift basket (manual machine)
    AwaitingSpinOff,
    /// Waiting for user to move to next jar (manual machine)
    AwaitingJar,
    /// Paused by user
    Paused,
    /// Step complete, transitioning
    StepComplete,
    /// All steps done
    Complete,
}

/// Current motor command from scheduler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MotorCommand {
    /// Target RPM (0 = stopped)
    pub rpm: u16,
    /// Rotation direction
    pub direction: Direction,
}

impl MotorCommand {
    /// Create a stopped command
    pub const fn stopped() -> Self {
        Self {
            rpm: 0,
            direction: Direction::Clockwise,
        }
    }

    /// Create a running command
    pub const fn running(rpm: u16, direction: Direction) -> Self {
        Self { rpm, direction }
    }
}

/// Current heater command from scheduler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HeaterCommand {
    /// Target temperature in °C (None = heater off)
    pub target_temp_c: Option<i16>,
}

impl HeaterCommand {
    /// Create an off command
    pub const fn off() -> Self {
        Self { target_temp_c: None }
    }

    /// Create a heating command
    pub const fn heating(temp_c: i16) -> Self {
        Self {
            target_temp_c: Some(temp_c),
        }
    }
}

/// Step execution state
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StepState {
    /// Current step index (0-based)
    pub step_index: u8,
    /// Total steps in program
    pub total_steps: u8,
    /// Jar index for current step
    pub jar_index: u8,
    /// Profile index for current step
    pub profile_index: u8,
    /// Segments for current step
    pub segments: Vec<Segment, MAX_SEGMENTS>,
    /// Current segment index
    pub segment_index: u8,
    /// Elapsed time in current segment (seconds)
    pub segment_elapsed_s: u16,
    /// Total elapsed time in current step (seconds)
    pub step_elapsed_s: u32,
    /// Spin-off configuration (if any)
    pub spinoff: Option<SpinOffConfig>,
    /// Spin-off elapsed time (seconds)
    pub spinoff_elapsed_s: u16,
}

impl Default for StepState {
    fn default() -> Self {
        Self {
            step_index: 0,
            total_steps: 0,
            jar_index: 0,
            profile_index: 0,
            segments: Vec::new(),
            segment_index: 0,
            segment_elapsed_s: 0,
            step_elapsed_s: 0,
            spinoff: None,
            spinoff_elapsed_s: 0,
        }
    }
}

/// Program scheduler
///
/// Tracks execution of a cleaning program and provides motor/heater commands.
#[derive(Debug)]
pub struct Scheduler {
    /// Current execution phase
    phase: ExecutionPhase,
    /// Machine capabilities (determines automated vs manual flow)
    capabilities: MachineCapabilities,
    /// Current step state
    step: StepState,
    /// Current program config (copied for duration of execution)
    program: Option<ProgramConfig>,
    /// Available profiles (referenced by name)
    profiles: Vec<ProfileConfig, MAX_PROFILES>,
    /// Available jars (referenced by name)
    jars: Vec<JarConfig, MAX_JARS>,
    /// Motor command state (preserved during pause)
    motor_cmd: MotorCommand,
    /// Heater command state
    heater_cmd: HeaterCommand,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(capabilities: MachineCapabilities) -> Self {
        Self {
            phase: ExecutionPhase::Idle,
            capabilities,
            step: StepState::default(),
            program: None,
            profiles: Vec::new(),
            jars: Vec::new(),
            motor_cmd: MotorCommand::stopped(),
            heater_cmd: HeaterCommand::off(),
        }
    }

    /// Load available profiles
    pub fn load_profiles(&mut self, profiles: &[ProfileConfig]) {
        self.profiles.clear();
        for p in profiles.iter().take(MAX_PROFILES) {
            let _ = self.profiles.push(p.clone());
        }
    }

    /// Load available jars
    pub fn load_jars(&mut self, jars: &[JarConfig]) {
        self.jars.clear();
        for j in jars.iter().take(MAX_JARS) {
            let _ = self.jars.push(j.clone());
        }
    }

    /// Get current execution phase
    pub fn phase(&self) -> ExecutionPhase {
        self.phase
    }

    /// Get current motor command
    pub fn motor_command(&self) -> MotorCommand {
        if matches!(
            self.phase,
            ExecutionPhase::Running | ExecutionPhase::SpinOff
        ) {
            self.motor_cmd
        } else {
            MotorCommand::stopped()
        }
    }

    /// Get current heater command
    pub fn heater_command(&self) -> HeaterCommand {
        if self.phase == ExecutionPhase::Running {
            self.heater_cmd
        } else {
            HeaterCommand::off()
        }
    }

    /// Get current step state (if running)
    pub fn step_state(&self) -> Option<&StepState> {
        if self.phase != ExecutionPhase::Idle && self.phase != ExecutionPhase::Complete {
            Some(&self.step)
        } else {
            None
        }
    }

    /// Start executing a program
    ///
    /// Returns the first event to send (or None if start failed)
    pub fn start_program(&mut self, program: ProgramConfig) -> Option<Event> {
        if program.steps.is_empty() {
            return None;
        }

        self.program = Some(program);
        self.step = StepState::default();

        // Try to start the first step
        self.start_step(0)
    }

    /// Start a specific step
    fn start_step(&mut self, step_index: u8) -> Option<Event> {
        let program = self.program.as_ref()?;

        if step_index as usize >= program.steps.len() {
            // No more steps
            self.phase = ExecutionPhase::Complete;
            return Some(Event::ProgramFinished);
        }

        let step = &program.steps[step_index as usize];

        // Find profile and jar by name
        let profile_index = self.find_profile(&step.profile)?;
        let jar_index = self.find_jar(&step.jar)?;

        let profile = &self.profiles[profile_index as usize];

        // Generate segments for this profile
        let segments = generate_segments(
            profile.rpm,
            profile.time_s,
            profile.direction,
            profile.iterations,
        )?;

        // Setup step state
        self.step = StepState {
            step_index,
            total_steps: program.steps.len() as u8,
            jar_index,
            profile_index,
            segments,
            segment_index: 0,
            segment_elapsed_s: 0,
            step_elapsed_s: 0,
            spinoff: profile.spinoff,
            spinoff_elapsed_s: 0,
        };

        // Setup motor command from first segment
        if let Some(seg) = self.step.segments.first() {
            self.motor_cmd = MotorCommand::running(seg.rpm, seg.direction);
        }

        // Setup heater command if profile has temperature target
        if let Some(temp) = profile.temperature_c {
            self.heater_cmd = HeaterCommand::heating(temp);
        } else {
            self.heater_cmd = HeaterCommand::off();
        }

        // For manual machines, prompt user to move to jar first
        if !self.capabilities.is_automated && step_index > 0 {
            self.phase = ExecutionPhase::AwaitingJar;
            return Some(Event::PromptNextJar);
        }

        self.phase = ExecutionPhase::Running;
        None
    }

    /// Find profile index by name
    fn find_profile(&self, name: &str) -> Option<u8> {
        self.profiles
            .iter()
            .position(|p| p.label.as_str() == name)
            .map(|i| i as u8)
    }

    /// Find jar index by name
    fn find_jar(&self, name: &str) -> Option<u8> {
        self.jars
            .iter()
            .position(|j| j.name.as_str() == name)
            .map(|i| i as u8)
    }

    /// Get current profile (if running)
    pub fn current_profile(&self) -> Option<&ProfileConfig> {
        if self.phase == ExecutionPhase::Idle {
            return None;
        }
        self.profiles.get(self.step.profile_index as usize)
    }

    /// Get current jar (if running)
    pub fn current_jar(&self) -> Option<&JarConfig> {
        if self.phase == ExecutionPhase::Idle {
            return None;
        }
        self.jars.get(self.step.jar_index as usize)
    }

    /// Update scheduler with elapsed time
    ///
    /// Call this periodically (e.g., every 100ms or 1s).
    /// Returns an event if a transition should occur.
    pub fn tick(&mut self, elapsed_s: u16) -> Option<Event> {
        match self.phase {
            ExecutionPhase::Running => self.tick_running(elapsed_s),
            ExecutionPhase::SpinOff => self.tick_spinoff(elapsed_s),
            _ => None,
        }
    }

    /// Tick while in Running phase
    fn tick_running(&mut self, elapsed_s: u16) -> Option<Event> {
        self.step.segment_elapsed_s += elapsed_s;
        self.step.step_elapsed_s += elapsed_s as u32;

        // Check if current segment is complete
        let segment = self.step.segments.get(self.step.segment_index as usize)?;
        if self.step.segment_elapsed_s >= segment.duration_s {
            // Move to next segment
            self.step.segment_index += 1;
            self.step.segment_elapsed_s = 0;

            if let Some(next_seg) = self.step.segments.get(self.step.segment_index as usize) {
                // Update motor command for new segment
                self.motor_cmd = MotorCommand::running(next_seg.rpm, next_seg.direction);
            } else {
                // All segments done, check for spin-off
                return self.finish_profile();
            }
        }

        None
    }

    /// Handle profile completion
    fn finish_profile(&mut self) -> Option<Event> {
        // Check if spin-off is configured
        if let Some(spinoff) = self.step.spinoff {
            // Setup for spin-off phase
            self.motor_cmd = MotorCommand::running(spinoff.rpm, Direction::Clockwise);
            self.heater_cmd = HeaterCommand::off(); // No heating during spin-off

            if self.capabilities.is_automated {
                // Automated: start spin-off immediately
                self.phase = ExecutionPhase::SpinOff;
                return Some(Event::StartSpinOff);
            } else {
                // Manual: prompt user to lift basket
                self.phase = ExecutionPhase::AwaitingSpinOff;
                return Some(Event::PromptSpinOff);
            }
        }

        // No spin-off, go directly to step complete
        self.finish_step()
    }

    /// Tick while in SpinOff phase
    fn tick_spinoff(&mut self, elapsed_s: u16) -> Option<Event> {
        self.step.spinoff_elapsed_s += elapsed_s;

        if let Some(spinoff) = self.step.spinoff {
            if self.step.spinoff_elapsed_s >= spinoff.time_s {
                // Spin-off complete
                return self.finish_spinoff();
            }
        }

        None
    }

    /// Handle spin-off completion
    fn finish_spinoff(&mut self) -> Option<Event> {
        self.motor_cmd = MotorCommand::stopped();
        self.finish_step()
    }

    /// Handle step completion
    fn finish_step(&mut self) -> Option<Event> {
        self.motor_cmd = MotorCommand::stopped();
        self.heater_cmd = HeaterCommand::off();

        let next_step = self.step.step_index + 1;
        let program = self.program.as_ref()?;

        if next_step as usize >= program.steps.len() {
            // All steps complete
            self.phase = ExecutionPhase::Complete;
            return Some(Event::ProgramFinished);
        }

        // More steps to go
        self.phase = ExecutionPhase::StepComplete;

        if self.capabilities.is_automated {
            // Automated machines advance automatically
            Some(Event::NextStep)
        } else {
            // Manual machines wait for user
            Some(Event::PromptNextJar)
        }
    }

    /// User confirmed (for manual machine prompts)
    pub fn user_confirm(&mut self) -> Option<Event> {
        match self.phase {
            ExecutionPhase::AwaitingJar => {
                self.phase = ExecutionPhase::Running;
                None
            }
            ExecutionPhase::AwaitingSpinOff => {
                self.phase = ExecutionPhase::SpinOff;
                None
            }
            _ => None,
        }
    }

    /// Advance to next step (after StepComplete)
    pub fn advance_step(&mut self) -> Option<Event> {
        if self.phase == ExecutionPhase::StepComplete {
            let next_index = self.step.step_index + 1;
            self.start_step(next_index)
        } else {
            None
        }
    }

    /// Pause execution
    ///
    /// Motor command is preserved for resume.
    pub fn pause(&mut self) -> bool {
        if self.phase == ExecutionPhase::Running || self.phase == ExecutionPhase::SpinOff {
            self.phase = ExecutionPhase::Paused;
            true
        } else {
            false
        }
    }

    /// Resume execution
    pub fn resume(&mut self) -> bool {
        if self.phase == ExecutionPhase::Paused {
            // Restore to running or spinoff based on whether we have spinoff time
            if self.step.spinoff.is_some() && self.step.spinoff_elapsed_s > 0 {
                self.phase = ExecutionPhase::SpinOff;
            } else {
                self.phase = ExecutionPhase::Running;
            }
            true
        } else {
            false
        }
    }

    /// Abort execution
    pub fn abort(&mut self) {
        self.phase = ExecutionPhase::Idle;
        self.motor_cmd = MotorCommand::stopped();
        self.heater_cmd = HeaterCommand::off();
        self.program = None;
        self.step = StepState::default();
    }

    /// Get total elapsed time for current program (seconds)
    pub fn total_elapsed_s(&self) -> u32 {
        // Sum of completed steps plus current step
        // (simplified: just return current step elapsed)
        self.step.step_elapsed_s
    }

    /// Get remaining time for current segment (seconds)
    pub fn segment_remaining_s(&self) -> u16 {
        if let Some(seg) = self.step.segments.get(self.step.segment_index as usize) {
            seg.duration_s.saturating_sub(self.step.segment_elapsed_s)
        } else {
            0
        }
    }

    /// Get total time for current step (seconds)
    pub fn step_total_s(&self) -> u32 {
        let profile_time: u32 = self.step.segments.iter().map(|s| s.duration_s as u32).sum();
        let spinoff_time = self
            .step
            .spinoff
            .map(|s| s.time_s as u32)
            .unwrap_or(0);
        profile_time + spinoff_time
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(MachineCapabilities::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::segment::DirectionMode;
    use heapless::String;

    fn make_profile(name: &str, rpm: u16, time_s: u16, direction: DirectionMode) -> ProfileConfig {
        let mut label = String::new();
        let _ = label.push_str(name);
        ProfileConfig {
            label,
            rpm,
            time_s,
            direction,
            iterations: 3,
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

    fn make_step(jar: &str, profile: &str) -> crate::config::ProgramStep {
        let mut j = String::new();
        let _ = j.push_str(jar);
        let mut p = String::new();
        let _ = p.push_str(profile);
        crate::config::ProgramStep { jar: j, profile: p }
    }

    fn make_program(name: &str, steps: &[(&str, &str)]) -> ProgramConfig {
        let mut label = String::new();
        let _ = label.push_str(name);
        let mut step_vec = heapless::Vec::new();
        for (jar, profile) in steps {
            let _ = step_vec.push(make_step(jar, profile));
        }
        ProgramConfig {
            label,
            steps: step_vec,
        }
    }

    #[test]
    fn test_scheduler_creation() {
        let sched = Scheduler::new(MachineCapabilities::default());
        assert_eq!(sched.phase(), ExecutionPhase::Idle);
        assert_eq!(sched.motor_command(), MotorCommand::stopped());
    }

    #[test]
    fn test_start_program() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        // Load profiles and jars
        let profiles = [make_profile("Clean", 120, 60, DirectionMode::Clockwise)];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        // Create and start program
        let program = make_program("Test", &[("clean", "Clean")]);
        let event = sched.start_program(program);

        // Should start running (automated machine, first step)
        assert!(event.is_none()); // No event needed to start
        assert_eq!(sched.phase(), ExecutionPhase::Running);

        // Motor should be running
        let cmd = sched.motor_command();
        assert_eq!(cmd.rpm, 120);
    }

    #[test]
    fn test_step_completion() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 10, DirectionMode::Clockwise)];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean")]);
        sched.start_program(program);

        // Tick through entire profile
        let event = sched.tick(15); // More than 10s

        // Should complete
        assert_eq!(event, Some(Event::ProgramFinished));
        assert_eq!(sched.phase(), ExecutionPhase::Complete);
    }

    #[test]
    fn test_pause_resume() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        let profiles = [make_profile("Clean", 120, 60, DirectionMode::Clockwise)];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean")]);
        sched.start_program(program);

        // Pause
        assert!(sched.pause());
        assert_eq!(sched.phase(), ExecutionPhase::Paused);
        assert_eq!(sched.motor_command(), MotorCommand::stopped());

        // Resume
        assert!(sched.resume());
        assert_eq!(sched.phase(), ExecutionPhase::Running);
        assert_eq!(sched.motor_command().rpm, 120);
    }

    #[test]
    fn test_manual_machine_flow() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: false, // Manual machine
            ..Default::default()
        });

        let profiles = [
            make_profile("Clean", 120, 10, DirectionMode::Clockwise),
            make_profile("Rinse", 100, 10, DirectionMode::Clockwise),
        ];
        let jars = [make_jar("clean"), make_jar("rinse")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean"), ("rinse", "Rinse")]);
        sched.start_program(program);

        // First step starts running (no prompt for first jar)
        assert_eq!(sched.phase(), ExecutionPhase::Running);

        // Complete first step
        let event = sched.tick(15);
        assert_eq!(event, Some(Event::PromptNextJar));
    }

    #[test]
    fn test_spinoff_flow() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: true,
            has_lift: true,
            ..Default::default()
        });

        let mut profile = make_profile("Clean", 120, 10, DirectionMode::Clockwise);
        profile.spinoff = Some(SpinOffConfig {
            lift_mm: 20,
            rpm: 150,
            time_s: 5,
        });

        let profiles = [profile];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean")]);
        sched.start_program(program);

        // Complete profile
        let event = sched.tick(15);
        assert_eq!(event, Some(Event::StartSpinOff));
        assert_eq!(sched.phase(), ExecutionPhase::SpinOff);

        // Motor should be at spinoff RPM
        assert_eq!(sched.motor_command().rpm, 150);

        // Complete spinoff
        let event = sched.tick(10);
        assert_eq!(event, Some(Event::ProgramFinished));
    }

    #[test]
    fn test_segment_tracking() {
        let mut sched = Scheduler::new(MachineCapabilities {
            is_automated: true,
            ..Default::default()
        });

        // Alternating profile: 3 iterations = 6 segments
        let profiles = [make_profile("Clean", 120, 60, DirectionMode::Alternate)];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean")]);
        sched.start_program(program);

        // Check segments were created
        let state = sched.step_state().unwrap();
        assert_eq!(state.segments.len(), 6);
        assert_eq!(state.segment_index, 0);

        // First segment is CW
        assert_eq!(state.segments[0].direction, Direction::Clockwise);
        assert_eq!(state.segments[1].direction, Direction::CounterClockwise);
    }

    #[test]
    fn test_abort() {
        let mut sched = Scheduler::new(MachineCapabilities::default());

        let profiles = [make_profile("Clean", 120, 60, DirectionMode::Clockwise)];
        let jars = [make_jar("clean")];
        sched.load_profiles(&profiles);
        sched.load_jars(&jars);

        let program = make_program("Test", &[("clean", "Clean")]);
        sched.start_program(program);

        sched.abort();

        assert_eq!(sched.phase(), ExecutionPhase::Idle);
        assert_eq!(sched.motor_command(), MotorCommand::stopped());
        assert!(sched.step_state().is_none());
    }
}
