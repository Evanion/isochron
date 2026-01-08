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
use isochron_core::safety::{SafetyMonitor, SafetyStatus};
use isochron_core::scheduler::{HeaterCommand, MotorCommand, Scheduler};
use isochron_core::state::{Event, State};
use isochron_protocol::InputEvent;

use heapless::Vec;

/// Controller state for coordinating subsystems
pub struct Controller {
    /// Current machine state
    state: State,
    /// Program scheduler
    scheduler: Scheduler,
    /// Safety monitor
    safety: SafetyMonitor,
    /// Available programs
    programs: Vec<ProgramConfig, MAX_PROGRAMS>,
    /// Available profiles (shared)
    profiles: Vec<ProfileConfig, MAX_PROFILES>,
    /// Available jars
    jars: Vec<JarConfig, MAX_JARS>,
    /// Currently selected program index
    selected_program: u8,
    /// Last tick timestamp (ms)
    last_tick_ms: u32,
}

impl Controller {
    /// Create a new controller with given capabilities
    pub fn new(capabilities: MachineCapabilities) -> Self {
        Self {
            state: State::Boot,
            scheduler: Scheduler::new(capabilities),
            safety: SafetyMonitor::new(),
            programs: Vec::new(),
            profiles: Vec::new(),
            jars: Vec::new(),
            selected_program: 0,
            last_tick_ms: 0,
        }
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
    pub fn boot_complete(&mut self) {
        self.transition(Event::BootComplete);
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
                // Navigate program list
                if !self.programs.is_empty() {
                    self.selected_program =
                        (self.selected_program + 1) % (self.programs.len() as u8);
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
                // Navigate program list backwards
                if !self.programs.is_empty() {
                    if self.selected_program == 0 {
                        self.selected_program = (self.programs.len() - 1) as u8;
                    } else {
                        self.selected_program -= 1;
                    }
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
                // Select program
                self.transition(Event::SelectProgram);
                Some(Event::SelectProgram)
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
                // Advance to next step
                if let Some(event) = self.scheduler.advance_step() {
                    self.transition(event);
                    Some(event)
                } else {
                    None
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
