//! Screen rendering
//!
//! Builds screens for different UI states.
//!
//! The V0 Display has an 128x64 OLED with 8 rows of 21 characters.
//! We use a simple text-based UI with inverted regions for selection.

use heapless::String;
use isochron_protocol::messages::{DISPLAY_COLS, DISPLAY_ROWS};

/// A screen buffer that can be sent to the display
pub struct Screen {
    /// Lines of text (8 rows max)
    lines: [String<22>; 8],
    /// Which row is currently selected (for menu highlighting)
    selected_row: Option<u8>,
    /// Whether to invert the selected row
    invert_selection: bool,
}

impl Screen {
    /// Create a new empty screen
    pub const fn new() -> Self {
        Self {
            lines: [
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            selected_row: None,
            invert_selection: false,
        }
    }

    /// Clear the screen
    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.selected_row = None;
        self.invert_selection = false;
    }

    /// Set text at a specific row
    pub fn set_line(&mut self, row: u8, text: &str) {
        if (row as usize) < self.lines.len() {
            self.lines[row as usize].clear();
            let _ =
                self.lines[row as usize].push_str(&text[..text.len().min(DISPLAY_COLS as usize)]);
        }
    }

    /// Set the selected row for highlighting
    pub fn set_selection(&mut self, row: u8, invert: bool) {
        if (row as usize) < DISPLAY_ROWS as usize {
            self.selected_row = Some(row);
            self.invert_selection = invert;
        }
    }

    /// Get a line of text
    pub fn get_line(&self, row: u8) -> &str {
        if (row as usize) < self.lines.len() {
            self.lines[row as usize].as_str()
        } else {
            ""
        }
    }

    /// Get selected row
    pub fn selected_row(&self) -> Option<u8> {
        self.selected_row
    }

    /// Check if selection should be inverted
    pub fn invert_selection(&self) -> bool {
        self.invert_selection
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

/// Screen renderer for different UI states
pub struct Renderer {
    screen: Screen,
}

impl Renderer {
    /// Create a new renderer
    pub const fn new() -> Self {
        Self {
            screen: Screen::new(),
        }
    }

    /// Get the current screen buffer
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Render the boot/connecting screen
    pub fn render_boot(&mut self) {
        self.screen.clear();
        self.screen.set_line(2, "    ISOCHRON");
        self.screen.set_line(4, "  Watch Cleaner");
        self.screen.set_line(6, " Connecting...");
    }

    /// Render the main menu
    ///
    /// # Arguments
    /// - `programs`: List of program names
    /// - `selected`: Currently selected index
    pub fn render_menu(&mut self, programs: &[&str], selected: usize) {
        self.screen.clear();
        self.screen.set_line(0, "=== SELECT PROGRAM ===");

        for (i, program) in programs.iter().take(6).enumerate() {
            let row = (i + 1) as u8;
            let mut line: String<22> = String::new();

            // Add selection indicator
            if i == selected {
                let _ = line.push_str("> ");
            } else {
                let _ = line.push_str("  ");
            }

            let _ = line.push_str(program);
            self.screen.set_line(row, &line);
        }

        if selected < 6 {
            self.screen.set_selection((selected + 1) as u8, true);
        }
    }

    /// Render program details screen
    ///
    /// # Arguments
    /// - `name`: Program name
    /// - `steps`: List of step descriptions (jar + profile)
    /// - `total_time_s`: Total estimated time in seconds
    pub fn render_program_detail(&mut self, name: &str, steps: &[&str], total_time_s: u32) {
        self.screen.clear();

        // Header
        let mut header: String<22> = String::new();
        let _ = header.push_str("= ");
        let _ = header.push_str(&name[..name.len().min(17)]);
        let _ = header.push_str(" =");
        self.screen.set_line(0, &header);

        // Steps
        for (i, step) in steps.iter().take(5).enumerate() {
            let row = (i + 1) as u8;
            let mut line: String<22> = String::new();
            let _ = write_to_string(&mut line, format_args!("{}. {}", i + 1, step));
            self.screen.set_line(row, &line);
        }

        // Total time
        let mins = total_time_s / 60;
        let secs = total_time_s % 60;
        let mut time_line: String<22> = String::new();
        let _ = write_to_string(&mut time_line, format_args!("Total: {}:{:02}", mins, secs));
        self.screen.set_line(6, &time_line);

        // Instructions
        self.screen.set_line(7, "CLICK=Start  <>=Back");
    }

    /// Render the running screen
    ///
    /// # Arguments
    /// - `program_name`: Current program name
    /// - `step_num`: Current step number (1-indexed)
    /// - `total_steps`: Total number of steps
    /// - `jar_name`: Current jar name
    /// - `profile_name`: Current profile name
    /// - `rpm`: Current motor RPM
    /// - `elapsed_s`: Elapsed time in seconds
    /// - `total_s`: Total time for this step in seconds
    /// - `temp_c`: Current temperature (None if no heater)
    /// - `target_c`: Target temperature (None if no heater)
    #[allow(clippy::too_many_arguments)]
    pub fn render_running(
        &mut self,
        program_name: &str,
        step_num: u8,
        total_steps: u8,
        jar_name: &str,
        profile_name: &str,
        rpm: u16,
        elapsed_s: u32,
        total_s: u32,
        temp_c: Option<i16>,
        target_c: Option<i16>,
    ) {
        self.screen.clear();

        // Header: program name
        self.screen.set_line(0, program_name);

        // Step info
        let mut step_line: String<22> = String::new();
        let _ = write_to_string(
            &mut step_line,
            format_args!("Step {}/{}: {}", step_num, total_steps, jar_name),
        );
        self.screen.set_line(1, &step_line);

        // Profile
        let mut profile_line: String<22> = String::new();
        let _ = write_to_string(&mut profile_line, format_args!("Profile: {}", profile_name));
        self.screen.set_line(2, &profile_line);

        // Motor status
        let mut motor_line: String<22> = String::new();
        let _ = write_to_string(&mut motor_line, format_args!("Motor: {} RPM", rpm));
        self.screen.set_line(3, &motor_line);

        // Temperature (if applicable)
        if let (Some(current), Some(target)) = (temp_c, target_c) {
            let mut temp_line: String<22> = String::new();
            let _ = write_to_string(
                &mut temp_line,
                format_args!("Temp: {}C / {}C", current, target),
            );
            self.screen.set_line(4, &temp_line);
        }

        // Progress bar
        let progress = if total_s > 0 {
            ((elapsed_s * 20) / total_s).min(20) as usize
        } else {
            0
        };
        let mut bar: String<22> = String::new();
        let _ = bar.push('[');
        for i in 0..20 {
            if i < progress {
                let _ = bar.push('#');
            } else {
                let _ = bar.push('-');
            }
        }
        let _ = bar.push(']');
        self.screen.set_line(5, &bar);

        // Time remaining
        let remaining = total_s.saturating_sub(elapsed_s);
        let mins = remaining / 60;
        let secs = remaining % 60;
        let mut time_line: String<22> = String::new();
        let _ = write_to_string(
            &mut time_line,
            format_args!("Remaining: {}:{:02}", mins, secs),
        );
        self.screen.set_line(6, &time_line);

        // Instructions
        self.screen.set_line(7, "CLICK=Pause");
    }

    /// Render the paused screen
    pub fn render_paused(&mut self, program_name: &str, step_num: u8, total_steps: u8) {
        self.screen.clear();
        self.screen.set_line(2, "    ** PAUSED **");

        let mut info_line: String<22> = String::new();
        let _ = write_to_string(
            &mut info_line,
            format_args!("{} ({}/{})", program_name, step_num, total_steps),
        );
        self.screen.set_line(4, &info_line);

        self.screen.set_line(6, "CLICK=Resume");
        self.screen.set_line(7, "HOLD=Abort");
    }

    /// Render the step complete screen (for manual machines)
    pub fn render_step_complete(&mut self, next_jar: &str) {
        self.screen.clear();
        self.screen.set_line(2, "  Step Complete!");
        self.screen.set_line(4, "Move basket to:");

        let mut jar_line: String<22> = String::new();
        let _ = write_to_string(&mut jar_line, format_args!("  -> {}", next_jar));
        self.screen.set_line(5, &jar_line);

        self.screen.set_line(7, "CLICK when ready");
    }

    /// Render the program complete screen
    pub fn render_complete(&mut self, program_name: &str, total_time_s: u32) {
        self.screen.clear();
        self.screen.set_line(1, "   ** COMPLETE **");

        let mut name_line: String<22> = String::new();
        let _ = write_to_string(&mut name_line, format_args!("  {}", program_name));
        self.screen.set_line(3, &name_line);

        let mins = total_time_s / 60;
        let secs = total_time_s % 60;
        let mut time_line: String<22> = String::new();
        let _ = write_to_string(&mut time_line, format_args!("  Time: {}:{:02}", mins, secs));
        self.screen.set_line(5, &time_line);

        self.screen.set_line(7, "CLICK to continue");
    }

    /// Render an error screen
    pub fn render_error(&mut self, error_type: &str, details: &str) {
        self.screen.clear();
        self.screen.set_line(0, "!!! ERROR !!!");
        self.screen.set_line(2, error_type);

        // Split details across multiple lines if needed
        let detail_bytes = details.as_bytes();
        for (i, chunk) in detail_bytes.chunks(21).enumerate().take(3) {
            if let Ok(s) = core::str::from_utf8(chunk) {
                self.screen.set_line((4 + i) as u8, s);
            }
        }

        self.screen.set_line(7, "Power cycle required");
    }

    /// Render awaiting jar screen (manual machine waiting for user)
    pub fn render_awaiting_jar(&mut self, jar_name: &str, action: &str) {
        self.screen.clear();
        self.screen.set_line(2, action);

        let mut jar_line: String<22> = String::new();
        let _ = write_to_string(&mut jar_line, format_args!("  -> {}", jar_name));
        self.screen.set_line(4, &jar_line);

        self.screen.set_line(7, "CLICK when ready");
    }

    /// Render autotune confirmation screen
    ///
    /// Shows target temperature and asks for confirmation.
    pub fn render_autotune_confirm(&mut self, target_c: i16) {
        self.screen.clear();
        self.screen.set_line(0, "=== HEATER AUTOTUNE ==");
        self.screen.set_line(2, "This will calibrate");
        self.screen.set_line(3, "PID coefficients.");

        let mut temp_line: String<22> = String::new();
        let _ = write_to_string(&mut temp_line, format_args!("Target: {}C", target_c));
        self.screen.set_line(5, &temp_line);

        self.screen.set_line(7, "CLICK=Start HOLD=Back");
    }

    /// Render autotune progress screen
    ///
    /// Shows oscillation count and elapsed time.
    pub fn render_autotune_progress(
        &mut self,
        peaks: u8,
        elapsed_s: u32,
        temp_c: i16,
        target_c: i16,
    ) {
        self.screen.clear();
        self.screen.set_line(0, "  AUTOTUNING...");

        let mut temp_line: String<22> = String::new();
        let _ = write_to_string(
            &mut temp_line,
            format_args!("Temp: {}C / {}C", temp_c, target_c),
        );
        self.screen.set_line(2, &temp_line);

        let mut peaks_line: String<22> = String::new();
        let _ = write_to_string(
            &mut peaks_line,
            format_args!("Oscillations: {}/12", peaks / 2),
        );
        self.screen.set_line(4, &peaks_line);

        let mins = elapsed_s / 60;
        let secs = elapsed_s % 60;
        let mut time_line: String<22> = String::new();
        let _ = write_to_string(
            &mut time_line,
            format_args!("Elapsed: {}:{:02}", mins, secs),
        );
        self.screen.set_line(5, &time_line);

        self.screen.set_line(7, "HOLD to cancel");
    }

    /// Render autotune complete screen
    ///
    /// Shows calculated PID coefficients.
    pub fn render_autotune_complete(&mut self, kp_x100: i16, ki_x100: i16, kd_x100: i16) {
        self.screen.clear();
        self.screen.set_line(0, " AUTOTUNE COMPLETE");

        let mut kp_line: String<22> = String::new();
        let _ = write_to_string(
            &mut kp_line,
            format_args!("Kp: {}.{:02}", kp_x100 / 100, (kp_x100 % 100).abs()),
        );
        self.screen.set_line(2, &kp_line);

        let mut ki_line: String<22> = String::new();
        let _ = write_to_string(
            &mut ki_line,
            format_args!("Ki: {}.{:02}", ki_x100 / 100, (ki_x100 % 100).abs()),
        );
        self.screen.set_line(3, &ki_line);

        let mut kd_line: String<22> = String::new();
        let _ = write_to_string(
            &mut kd_line,
            format_args!("Kd: {}.{:02}", kd_x100 / 100, (kd_x100 % 100).abs()),
        );
        self.screen.set_line(4, &kd_line);

        self.screen.set_line(6, "Coefficients saved!");
        self.screen.set_line(7, "CLICK to continue");
    }

    /// Render autotune failed screen
    pub fn render_autotune_failed(&mut self, reason: &str) {
        self.screen.clear();
        self.screen.set_line(0, "  AUTOTUNE FAILED");
        self.screen.set_line(3, reason);
        self.screen.set_line(7, "CLICK to continue");
    }

    // --- Position state screens (for automated machines) ---

    /// Render homing screen
    ///
    /// Shows which axis is being homed.
    pub fn render_homing(&mut self, axis: &str) {
        self.screen.clear();
        self.screen.set_line(2, "     HOMING");

        let mut axis_line: String<22> = String::new();
        let _ = write_to_string(&mut axis_line, format_args!("  {} axis...", axis));
        self.screen.set_line(4, &axis_line);

        self.screen.set_line(6, "Please wait");
    }

    /// Render lifting screen
    ///
    /// Shows Z-axis lifting to safe position.
    pub fn render_lifting(&mut self) {
        self.screen.clear();
        self.screen.set_line(2, "   LIFTING BASKET");
        self.screen.set_line(4, " Moving to safe Z...");
        self.screen.set_line(6, "Please wait");
    }

    /// Render moving to jar screen
    ///
    /// Shows X-axis moving to target jar position.
    pub fn render_moving_to_jar(&mut self, jar_name: &str) {
        self.screen.clear();
        self.screen.set_line(2, "  MOVING TO JAR");

        let mut jar_line: String<22> = String::new();
        let _ = write_to_string(&mut jar_line, format_args!("  -> {}", jar_name));
        self.screen.set_line(4, &jar_line);

        self.screen.set_line(6, "Please wait");
    }

    /// Render lowering screen
    ///
    /// Shows Z-axis lowering into jar.
    pub fn render_lowering(&mut self, jar_name: &str) {
        self.screen.clear();
        self.screen.set_line(2, " LOWERING BASKET");

        let mut jar_line: String<22> = String::new();
        let _ = write_to_string(&mut jar_line, format_args!("  Into: {}", jar_name));
        self.screen.set_line(4, &jar_line);

        self.screen.set_line(6, "Please wait");
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to write formatted output to a heapless String
fn write_to_string(s: &mut String<22>, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
    use core::fmt::Write;
    s.write_fmt(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_basic() {
        let mut screen = Screen::new();
        screen.set_line(0, "Hello");
        assert_eq!(screen.get_line(0), "Hello");
    }

    #[test]
    fn test_screen_clear() {
        let mut screen = Screen::new();
        screen.set_line(0, "Hello");
        screen.set_selection(0, true);
        screen.clear();
        assert_eq!(screen.get_line(0), "");
        assert!(screen.selected_row().is_none());
    }

    #[test]
    fn test_render_boot() {
        let mut renderer = Renderer::new();
        renderer.render_boot();
        assert!(renderer.screen().get_line(2).contains("ISOCHRON"));
    }

    #[test]
    fn test_render_menu() {
        let mut renderer = Renderer::new();
        let programs = ["Full Clean", "Quick Clean", "Dry Only"];
        renderer.render_menu(&programs, 1);

        // Selected item should have indicator
        assert!(renderer.screen().get_line(2).starts_with(">"));
        assert_eq!(renderer.screen().selected_row(), Some(2));
    }

    #[test]
    fn test_render_running() {
        let mut renderer = Renderer::new();
        renderer.render_running(
            "Full Clean",
            1,
            4,
            "clean",
            "Clean",
            120,
            30,
            180,
            Some(42),
            Some(45),
        );

        assert!(renderer.screen().get_line(0).contains("Full Clean"));
        assert!(renderer.screen().get_line(3).contains("120 RPM"));
    }

    #[test]
    fn test_render_error() {
        let mut renderer = Renderer::new();
        renderer.render_error("OVER TEMP", "Temperature exceeded 55C");

        assert!(renderer.screen().get_line(0).contains("ERROR"));
        assert!(renderer.screen().get_line(2).contains("OVER TEMP"));
    }
}
