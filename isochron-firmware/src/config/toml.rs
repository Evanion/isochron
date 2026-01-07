//! Simple TOML parser for machine configuration
//!
//! This is a minimal TOML parser that handles only the subset needed for
//! Isochron configuration. It does NOT support the full TOML spec.
//!
//! Supported features:
//! - Key = value pairs (string, integer, boolean)
//! - [section] headers
//! - [section.subsection] headers
//! - Inline tables for arrays: steps = [{ jar = "x", profile = "y" }]
//! - Comments (# ...)
//!
//! NOT supported:
//! - Multi-line strings
//! - Datetime values
//! - Nested inline tables
//! - Dotted keys outside section headers

use alloc::vec::Vec;
use heapless::String as HString;

use isochron_core::config::{
    DisplayHwConfig, HeaterConfig, HeaterControlMode, HeaterHwConfig, JarConfig, MachineConfig,
    PinConfig, ProfileConfig, ProfileType, ProgramConfig, ProgramStep, SensorType,
    StepperHwConfig, Tmc2209HwConfig, UiConfig, MAX_LABEL_LEN,
};
use isochron_core::scheduler::{DirectionMode, SpinOffConfig};

/// Parse error
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ParseError {
    /// Invalid section header
    InvalidSection,
    /// Invalid value type
    InvalidValue,
    /// Too many items (exceeded heapless capacity)
    TooManyItems,
    /// Invalid pin string
    InvalidPin,
}

/// Current parsing context
#[derive(Debug, Clone)]
#[allow(dead_code)] // ProfileSpinoff name field reserved for future use
enum Section {
    Root,
    Stepper(HString<MAX_LABEL_LEN>),
    Tmc2209(HString<MAX_LABEL_LEN>),
    Heater(HString<MAX_LABEL_LEN>),
    HeaterHw(HString<MAX_LABEL_LEN>),
    Jar(HString<MAX_LABEL_LEN>),
    Profile(HString<MAX_LABEL_LEN>),
    ProfileSpinoff(HString<MAX_LABEL_LEN>),
    Program(HString<MAX_LABEL_LEN>),
    Display,
    Ui,
}

/// Parse TOML configuration into MachineConfig
pub fn parse_config(input: &str) -> Result<MachineConfig, ParseError> {
    let mut config = MachineConfig::new();
    let mut section = Section::Root;

    // Temporary storage for current section being built
    let mut current_stepper: Option<StepperHwConfig> = None;
    let mut current_tmc: Option<Tmc2209HwConfig> = None;
    let mut current_heater: Option<HeaterConfig> = None;
    let mut current_heater_hw: Option<HeaterHwConfig> = None;
    let mut current_jar: Option<JarConfig> = None;
    let mut current_profile: Option<ProfileConfig> = None;
    let mut current_spinoff: Option<SpinOffConfig> = None;
    let mut current_program: Option<ProgramConfig> = None;

    for line in input.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check for section header
        if line.starts_with('[') && line.ends_with(']') {
            // Save previous section
            save_section(
                &section,
                &mut config,
                &mut current_stepper,
                &mut current_tmc,
                &mut current_heater,
                &mut current_heater_hw,
                &mut current_jar,
                &mut current_profile,
                &mut current_spinoff,
                &mut current_program,
            )?;

            // Parse new section
            section = parse_section_header(&line[1..line.len() - 1])?;

            // Initialize new section
            match &section {
                Section::Stepper(name) => {
                    let mut s = StepperHwConfig::default();
                    s.name = name.clone();
                    current_stepper = Some(s);
                }
                Section::Tmc2209(name) => {
                    let mut t = Tmc2209HwConfig::default();
                    t.stepper_name = name.clone();
                    current_tmc = Some(t);
                }
                Section::Heater(name) => {
                    let mut h = HeaterConfig::default();
                    h.name = name.clone();
                    current_heater = Some(h);
                }
                Section::HeaterHw(name) => {
                    let mut h = HeaterHwConfig::default();
                    h.name = name.clone();
                    current_heater_hw = Some(h);
                }
                Section::Jar(name) => {
                    let mut j = JarConfig::default();
                    j.name = name.clone();
                    current_jar = Some(j);
                }
                Section::Profile(name) => {
                    let mut p = ProfileConfig::default();
                    p.label = name.clone();
                    current_profile = Some(p);
                    current_spinoff = None;
                }
                Section::ProfileSpinoff(_) => {
                    current_spinoff = Some(SpinOffConfig {
                        lift_mm: 20,
                        rpm: 150,
                        time_s: 10,
                    });
                }
                Section::Program(name) => {
                    let mut p = ProgramConfig::default();
                    p.label = name.clone();
                    current_program = Some(p);
                }
                Section::Display => {
                    config.display = DisplayHwConfig::default();
                }
                Section::Ui => {
                    config.ui = UiConfig::default();
                }
                Section::Root => {}
            }
            continue;
        }

        // Parse key = value
        if let Some((key, value)) = parse_key_value(line) {
            apply_value(
                &section,
                key,
                value,
                &mut config,
                &mut current_stepper,
                &mut current_tmc,
                &mut current_heater,
                &mut current_heater_hw,
                &mut current_jar,
                &mut current_profile,
                &mut current_spinoff,
                &mut current_program,
            )?;
        }
    }

    // Save final section
    save_section(
        &section,
        &mut config,
        &mut current_stepper,
        &mut current_tmc,
        &mut current_heater,
        &mut current_heater_hw,
        &mut current_jar,
        &mut current_profile,
        &mut current_spinoff,
        &mut current_program,
    )?;

    Ok(config)
}

/// Parse section header like "stepper spin", "stepper.spin" or "profile.clean.spinoff"
fn parse_section_header(header: &str) -> Result<Section, ParseError> {
    let header = header.trim();

    // Check for dotted sections (stepper.spin, profile.clean.spinoff, etc.)
    if header.contains('.') {
        let parts: Vec<&str> = header.split('.').collect();

        // Handle 3-part dotted sections (profile.name.spinoff)
        if parts.len() == 3 && parts[0] == "profile" && parts[2] == "spinoff" {
            let name = HString::try_from(parts[1]).map_err(|_| ParseError::InvalidSection)?;
            return Ok(Section::ProfileSpinoff(name));
        }

        // Handle 2-part dotted sections (stepper.spin, tmc2209.spin, etc.)
        if parts.len() == 2 {
            let section_type = parts[0];
            let name_str = parts[1];
            let name = HString::try_from(name_str).map_err(|_| ParseError::InvalidSection)?;

            return match section_type {
                "stepper" => Ok(Section::Stepper(name)),
                "tmc2209" => Ok(Section::Tmc2209(name)),
                "heater" => Ok(Section::HeaterHw(name)),
                "heater_control" => Ok(Section::Heater(name)),
                "jar" => Ok(Section::Jar(name)),
                "profile" => Ok(Section::Profile(name)),
                "program" => Ok(Section::Program(name)),
                _ => Err(ParseError::InvalidSection),
            };
        }

        return Err(ParseError::InvalidSection);
    }

    // Split on whitespace for "type name" format (legacy support)
    let mut parts = header.split_whitespace();
    let section_type = parts.next().ok_or(ParseError::InvalidSection)?;
    let name = parts.next();

    match section_type {
        "stepper" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Stepper(name))
        }
        "tmc2209" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Tmc2209(name))
        }
        "heater" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            // Determine if this is HeaterConfig or HeaterHwConfig based on first key
            // For now, assume it's HeaterHwConfig (hardware config)
            Ok(Section::HeaterHw(name))
        }
        "heater_control" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Heater(name))
        }
        "jar" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Jar(name))
        }
        "profile" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Profile(name))
        }
        "program" => {
            let name = name.ok_or(ParseError::InvalidSection)?;
            let name = HString::try_from(name).map_err(|_| ParseError::InvalidSection)?;
            Ok(Section::Program(name))
        }
        "display" => Ok(Section::Display),
        "ui" => Ok(Section::Ui),
        _ => Err(ParseError::InvalidSection),
    }
}

/// Parse "key = value" line
fn parse_key_value(line: &str) -> Option<(&str, &str)> {
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim();
    let value = line[eq_pos + 1..].trim();

    // Remove inline comments
    let value = if let Some(hash_pos) = value.find('#') {
        // Make sure # is not inside a string
        let quote_count = value[..hash_pos].matches('"').count();
        if quote_count % 2 == 0 {
            value[..hash_pos].trim()
        } else {
            value
        }
    } else {
        value
    };

    if key.is_empty() || value.is_empty() {
        return None;
    }

    Some((key, value))
}

/// Parse a string value (removes quotes)
fn parse_string(value: &str) -> Result<&str, ParseError> {
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Ok(&value[1..value.len() - 1])
    } else {
        // Allow unquoted strings for simple values
        Ok(value)
    }
}

/// Parse an integer value
fn parse_int<T: core::str::FromStr>(value: &str) -> Result<T, ParseError> {
    value.parse().map_err(|_| ParseError::InvalidValue)
}

/// Parse a boolean value
fn parse_bool(value: &str) -> Result<bool, ParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ParseError::InvalidValue),
    }
}

/// Parse a pin string like "gpio11", "!gpio12", "^gpio4"
fn parse_pin(value: &str) -> Result<PinConfig, ParseError> {
    let value = parse_string(value)?;
    let mut inverted = false;
    let mut pull_up = false;
    let mut s = value;

    // Check for modifiers
    while !s.is_empty() {
        if s.starts_with('!') {
            inverted = true;
            s = &s[1..];
        } else if s.starts_with('^') {
            pull_up = true;
            s = &s[1..];
        } else {
            break;
        }
    }

    // Parse "gpioNN"
    if !s.starts_with("gpio") {
        return Err(ParseError::InvalidPin);
    }

    let pin_num: u8 = s[4..].parse().map_err(|_| ParseError::InvalidPin)?;

    Ok(PinConfig {
        pin: pin_num,
        inverted,
        pull_up,
    })
}

/// Parse direction mode
fn parse_direction(value: &str) -> Result<DirectionMode, ParseError> {
    let value = parse_string(value)?;
    match value {
        "cw" | "clockwise" | "Clockwise" => Ok(DirectionMode::Clockwise),
        "ccw" | "counterclockwise" | "CounterClockwise" => Ok(DirectionMode::CounterClockwise),
        "alternate" | "Alternate" => Ok(DirectionMode::Alternate),
        _ => Err(ParseError::InvalidValue),
    }
}

/// Parse profile type
fn parse_profile_type(value: &str) -> Result<ProfileType, ParseError> {
    let value = parse_string(value)?;
    match value {
        "clean" | "Clean" => Ok(ProfileType::Clean),
        "rinse" | "Rinse" => Ok(ProfileType::Rinse),
        "dry" | "Dry" => Ok(ProfileType::Dry),
        _ => Err(ParseError::InvalidValue),
    }
}

/// Parse sensor type
fn parse_sensor_type(value: &str) -> Result<SensorType, ParseError> {
    let value = parse_string(value)?;
    match value {
        "ntc100k" | "NTC100K" => Ok(SensorType::Ntc100k),
        "ntc10k" | "NTC10K" => Ok(SensorType::Ntc10k),
        "pt100" | "PT100" => Ok(SensorType::Pt100),
        _ => Err(ParseError::InvalidValue),
    }
}

/// Parse heater control mode
fn parse_control_mode(value: &str) -> Result<HeaterControlMode, ParseError> {
    let value = parse_string(value)?;
    match value {
        "bang_bang" | "BangBang" => Ok(HeaterControlMode::BangBang),
        "pid" | "PID" => Ok(HeaterControlMode::Pid),
        _ => Err(ParseError::InvalidValue),
    }
}

/// Parse gear ratio string like "3:1"
fn parse_gear_ratio(value: &str) -> Result<(u8, u8), ParseError> {
    let value = parse_string(value)?;
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidValue);
    }
    let num: u8 = parts[0].parse().map_err(|_| ParseError::InvalidValue)?;
    let den: u8 = parts[1].parse().map_err(|_| ParseError::InvalidValue)?;
    Ok((num, den))
}

/// Parse program steps array
fn parse_steps(value: &str) -> Result<heapless::Vec<ProgramStep, 8>, ParseError> {
    let mut steps = heapless::Vec::new();

    // Remove outer brackets
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(ParseError::InvalidValue);
    }
    let inner = &value[1..value.len() - 1];

    // Parse each step { jar = "x", profile = "y" }
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in inner.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let step_str = &inner[start..=i];
                    if let Some(step) = parse_single_step(step_str) {
                        steps.push(step).map_err(|_| ParseError::TooManyItems)?;
                    }
                    start = i + 1;
                }
            }
            _ => {}
        }
    }

    Ok(steps)
}

/// Parse a single step like { jar = "clean", profile = "Clean" }
fn parse_single_step(s: &str) -> Option<ProgramStep> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    let inner = &s[1..s.len() - 1];

    let mut jar = None;
    let mut profile = None;

    for part in inner.split(',') {
        let part = part.trim();
        if let Some((key, value)) = parse_key_value(part) {
            let value = parse_string(value).ok()?;
            match key {
                "jar" => {
                    jar = Some(HString::try_from(value).ok()?);
                }
                "profile" => {
                    profile = Some(HString::try_from(value).ok()?);
                }
                _ => {}
            }
        }
    }

    Some(ProgramStep {
        jar: jar?,
        profile: profile?,
    })
}

/// Apply a parsed value to the appropriate config field
#[allow(clippy::too_many_arguments)]
fn apply_value(
    section: &Section,
    key: &str,
    value: &str,
    config: &mut MachineConfig,
    current_stepper: &mut Option<StepperHwConfig>,
    current_tmc: &mut Option<Tmc2209HwConfig>,
    current_heater: &mut Option<HeaterConfig>,
    current_heater_hw: &mut Option<HeaterHwConfig>,
    current_jar: &mut Option<JarConfig>,
    current_profile: &mut Option<ProfileConfig>,
    current_spinoff: &mut Option<SpinOffConfig>,
    current_program: &mut Option<ProgramConfig>,
) -> Result<(), ParseError> {
    match section {
        Section::Stepper(_) => {
            let s = current_stepper.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "step_pin" => s.step_pin = parse_pin(value)?,
                "dir_pin" => s.dir_pin = parse_pin(value)?,
                "enable_pin" => s.enable_pin = parse_pin(value)?,
                "endstop_pin" => s.endstop_pin = Some(parse_pin(value)?),
                "full_steps_per_rotation" => s.full_steps_per_rotation = parse_int(value)?,
                "microsteps" => s.microsteps = parse_int(value)?,
                "rotation_distance" => s.rotation_distance = parse_int(value)?,
                "gear_ratio" => {
                    let (num, den) = parse_gear_ratio(value)?;
                    s.gear_ratio_num = num;
                    s.gear_ratio_den = den;
                }
                _ => {} // Ignore unknown keys
            }
        }
        Section::Tmc2209(_) => {
            let t = current_tmc.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "uart_tx_pin" | "tx_pin" => {
                    let pin = parse_pin(value)?;
                    t.uart_tx_pin = pin.pin;
                }
                "uart_rx_pin" | "rx_pin" => {
                    let pin = parse_pin(value)?;
                    t.uart_rx_pin = pin.pin;
                }
                "uart_address" => t.uart_address = parse_int(value)?,
                "run_current" | "run_current_ma" => {
                    // Support both decimal amps and integer mA
                    if value.contains('.') {
                        let amps: f32 = value.parse().map_err(|_| ParseError::InvalidValue)?;
                        t.run_current_ma = (amps * 1000.0) as u16;
                    } else {
                        t.run_current_ma = parse_int(value)?;
                    }
                }
                "hold_current" | "hold_current_ma" => {
                    if value.contains('.') {
                        let amps: f32 = value.parse().map_err(|_| ParseError::InvalidValue)?;
                        t.hold_current_ma = (amps * 1000.0) as u16;
                    } else {
                        t.hold_current_ma = parse_int(value)?;
                    }
                }
                "stealthchop" => t.stealthchop = parse_bool(value)?,
                "stallguard_threshold" | "stall_threshold" => t.stall_threshold = parse_int(value)?,
                "diag_pin" => {
                    let pin = parse_pin(value)?;
                    t.diag_pin = Some(pin.pin);
                }
                _ => {}
            }
        }
        Section::Heater(_) => {
            let h = current_heater.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "control" => h.control = parse_control_mode(value)?,
                "max_temp" => h.max_temp = parse_int(value)?,
                "hysteresis" => h.hysteresis = parse_int(value)?,
                _ => {}
            }
        }
        Section::HeaterHw(_) => {
            let h = current_heater_hw.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "heater_pin" => h.heater_pin = parse_pin(value)?,
                "sensor_pin" => {
                    let pin = parse_pin(value)?;
                    h.sensor_pin = pin.pin;
                }
                "sensor_type" => h.sensor_type = parse_sensor_type(value)?,
                // Also handle control params in hardware section
                "control" | "max_temp" | "hysteresis" => {
                    // These belong to HeaterConfig, but users might put them here
                    // We'll handle this by also creating a HeaterConfig
                }
                _ => {}
            }
        }
        Section::Jar(_) => {
            let j = current_jar.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "tower_pos" => j.tower_pos = parse_int(value)?,
                "lift_pos" => j.lift_pos = parse_int(value)?,
                "heater" => {
                    let name = parse_string(value)?;
                    j.heater = Some(HString::try_from(name).map_err(|_| ParseError::InvalidValue)?);
                }
                "ultrasonic" => {
                    let name = parse_string(value)?;
                    j.ultrasonic =
                        Some(HString::try_from(name).map_err(|_| ParseError::InvalidValue)?);
                }
                _ => {}
            }
        }
        Section::Profile(_) => {
            let p = current_profile.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "label" => {
                    let label = parse_string(value)?;
                    p.label = HString::try_from(label).map_err(|_| ParseError::InvalidValue)?;
                }
                "type" => p.profile_type = parse_profile_type(value)?,
                "rpm" => p.rpm = parse_int(value)?,
                "time_s" => p.time_s = parse_int(value)?,
                "direction" => p.direction = parse_direction(value)?,
                "iterations" => p.iterations = parse_int(value)?,
                "temperature_c" => p.temperature_c = Some(parse_int(value)?),
                _ => {}
            }
        }
        Section::ProfileSpinoff(_) => {
            let s = current_spinoff.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "lift_mm" => s.lift_mm = parse_int(value)?,
                "rpm" => s.rpm = parse_int(value)?,
                "time_s" => s.time_s = parse_int(value)?,
                _ => {}
            }
        }
        Section::Program(_) => {
            let p = current_program.as_mut().ok_or(ParseError::InvalidSection)?;
            match key {
                "label" => {
                    let label = parse_string(value)?;
                    p.label = HString::try_from(label).map_err(|_| ParseError::InvalidValue)?;
                }
                "steps" => {
                    p.steps = parse_steps(value)?;
                }
                _ => {}
            }
        }
        Section::Display => {
            match key {
                "uart_tx_pin" | "tx_pin" => {
                    let pin = parse_pin(value)?;
                    config.display.uart_tx_pin = pin.pin;
                }
                "uart_rx_pin" | "rx_pin" => {
                    let pin = parse_pin(value)?;
                    config.display.uart_rx_pin = pin.pin;
                }
                "baud" | "baud_rate" => config.display.baud_rate = parse_int(value)?,
                _ => {}
            }
        }
        Section::Ui => match key {
            "rpm_step" => config.ui.rpm_step = parse_int(value)?,
            "time_step_s" => config.ui.time_step_s = parse_int(value)?,
            "temp_step_c" => config.ui.temp_step_c = parse_int(value)?,
            _ => {}
        },
        Section::Root => {
            // Handle root-level keys if any
        }
    }

    Ok(())
}

/// Save current section to config
#[allow(clippy::too_many_arguments)]
fn save_section(
    section: &Section,
    config: &mut MachineConfig,
    current_stepper: &mut Option<StepperHwConfig>,
    current_tmc: &mut Option<Tmc2209HwConfig>,
    current_heater: &mut Option<HeaterConfig>,
    current_heater_hw: &mut Option<HeaterHwConfig>,
    current_jar: &mut Option<JarConfig>,
    current_profile: &mut Option<ProfileConfig>,
    current_spinoff: &mut Option<SpinOffConfig>,
    current_program: &mut Option<ProgramConfig>,
) -> Result<(), ParseError> {
    match section {
        Section::Stepper(_) => {
            if let Some(s) = current_stepper.take() {
                config
                    .steppers
                    .push(s)
                    .map_err(|_| ParseError::TooManyItems)?;
            }
        }
        Section::Tmc2209(_) => {
            if let Some(t) = current_tmc.take() {
                config
                    .tmc2209s
                    .push(t)
                    .map_err(|_| ParseError::TooManyItems)?;
            }
        }
        Section::Heater(_) => {
            if let Some(h) = current_heater.take() {
                config
                    .heaters
                    .push(h)
                    .map_err(|_| ParseError::TooManyItems)?;
            }
        }
        Section::HeaterHw(_) => {
            if let Some(h) = current_heater_hw.take() {
                // Also create a HeaterConfig with defaults if not already present
                let name = h.name.clone();
                config
                    .heater_hw
                    .push(h)
                    .map_err(|_| ParseError::TooManyItems)?;

                // Check if we need to create a default HeaterConfig
                if !config.heaters.iter().any(|hc| hc.name == name) {
                    let mut hc = HeaterConfig::default();
                    hc.name = name;
                    config
                        .heaters
                        .push(hc)
                        .map_err(|_| ParseError::TooManyItems)?;
                }
            }
        }
        Section::Jar(_) => {
            if let Some(j) = current_jar.take() {
                config
                    .jars
                    .push(j)
                    .map_err(|_| ParseError::TooManyItems)?;
            }
        }
        Section::Profile(_) | Section::ProfileSpinoff(_) => {
            // Attach spinoff to profile if present
            if let Some(spinoff) = current_spinoff.take() {
                if let Some(ref mut p) = current_profile {
                    p.spinoff = Some(spinoff);
                }
            }

            // Save profile when moving to new section (not spinoff subsection)
            if !matches!(section, Section::ProfileSpinoff(_)) {
                if let Some(p) = current_profile.take() {
                    config
                        .profiles
                        .push(p)
                        .map_err(|_| ParseError::TooManyItems)?;
                }
            }
        }
        Section::Program(_) => {
            if let Some(p) = current_program.take() {
                config
                    .programs
                    .push(p)
                    .map_err(|_| ParseError::TooManyItems)?;
            }
        }
        Section::Display | Section::Ui | Section::Root => {
            // These are stored directly in config, nothing to save
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pin() {
        let pin = parse_pin("gpio11").unwrap();
        assert_eq!(pin.pin, 11);
        assert!(!pin.inverted);
        assert!(!pin.pull_up);

        let pin = parse_pin("!gpio12").unwrap();
        assert_eq!(pin.pin, 12);
        assert!(pin.inverted);

        let pin = parse_pin("^gpio4").unwrap();
        assert_eq!(pin.pin, 4);
        assert!(pin.pull_up);

        let pin = parse_pin("\"^!gpio5\"").unwrap();
        assert_eq!(pin.pin, 5);
        assert!(pin.inverted);
        assert!(pin.pull_up);
    }

    #[test]
    fn test_parse_section_header() {
        match parse_section_header("stepper spin").unwrap() {
            Section::Stepper(name) => assert_eq!(name.as_str(), "spin"),
            _ => panic!("Wrong section type"),
        }

        match parse_section_header("profile.clean.spinoff").unwrap() {
            Section::ProfileSpinoff(name) => assert_eq!(name.as_str(), "clean"),
            _ => panic!("Wrong section type"),
        }

        match parse_section_header("display").unwrap() {
            Section::Display => {}
            _ => panic!("Wrong section type"),
        }
    }

    #[test]
    fn test_parse_gear_ratio() {
        let (num, den) = parse_gear_ratio("\"3:1\"").unwrap();
        assert_eq!(num, 3);
        assert_eq!(den, 1);

        let (num, den) = parse_gear_ratio("5:2").unwrap();
        assert_eq!(num, 5);
        assert_eq!(den, 2);
    }

    #[test]
    fn test_parse_steps() {
        let steps_str =
            r#"[{ jar = "clean", profile = "Clean" }, { jar = "rinse", profile = "Rinse" }]"#;
        let steps = parse_steps(steps_str).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].jar.as_str(), "clean");
        assert_eq!(steps[0].profile.as_str(), "Clean");
        assert_eq!(steps[1].jar.as_str(), "rinse");
        assert_eq!(steps[1].profile.as_str(), "Rinse");
    }

    #[test]
    fn test_parse_minimal_config() {
        let config_str = r#"
[stepper spin]
step_pin = "gpio11"
dir_pin = "gpio10"
enable_pin = "!gpio12"

[display]
uart_tx_pin = "gpio0"
uart_rx_pin = "gpio1"
"#;

        let config = parse_config(config_str).unwrap();
        assert_eq!(config.steppers.len(), 1);
        assert_eq!(config.steppers[0].name.as_str(), "spin");
        assert_eq!(config.steppers[0].step_pin.pin, 11);
        assert!(config.steppers[0].enable_pin.inverted);
        assert_eq!(config.display.uart_tx_pin, 0);
    }
}
