//! Build script for isochron-firmware
//!
//! - Sets up linker search paths for memory.x
//! - Validates machine.toml at compile time

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    setup_linker();
    validate_config();
}

/// Set up linker search paths for memory.x
fn setup_linker() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Copy memory.x to the output directory
    let memory_x = include_bytes!("memory.x");
    let mut f = File::create(out_dir.join("memory.x")).unwrap();
    f.write_all(memory_x).unwrap();

    // Tell rustc where to find memory.x
    println!("cargo:rustc-link-search={}", out_dir.display());

    // Re-run if memory.x changes
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Validate machine.toml configuration at compile time
fn validate_config() {
    // Re-run if machine.toml changes
    println!("cargo:rerun-if-changed=machine.toml");

    let config_path = Path::new("machine.toml");

    // Check if config file exists
    if !config_path.exists() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: machine.toml not found!                                  ║\n\
            ║                                                                  ║\n\
            ║  The firmware requires a machine.toml configuration file.        ║\n\
            ║  Please create one in the isochron-firmware directory.           ║\n\
            ║                                                                  ║\n\
            ║  See docs/Config_Reference.md for configuration options.         ║\n\
            ╚══════════════════════════════════════════════════════════════════╝\n"
        );
    }

    // Read the config file
    let config_content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(e) => {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════╗\n\
                ║  ERROR: Failed to read machine.toml                              ║\n\
                ║                                                                  ║\n\
                ║  Error: {:<56} ║\n\
                ╚══════════════════════════════════════════════════════════════════╝\n",
                e
            );
        }
    };

    // Parse and validate TOML syntax
    let config: toml::Value = match toml::from_str(&config_content) {
        Ok(value) => value,
        Err(e) => {
            let error_msg = e.to_string();
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════╗\n\
                ║  ERROR: Invalid TOML syntax in machine.toml                      ║\n\
                ╠══════════════════════════════════════════════════════════════════╣\n\
                ║                                                                  ║\n\
                {}\n\
                ║                                                                  ║\n\
                ╚══════════════════════════════════════════════════════════════════╝\n",
                format_error_lines(&error_msg)
            );
        }
    };

    // Validate required sections exist
    validate_required_sections(&config);

    // Validate section contents
    validate_machine_section(&config);
    validate_profiles(&config);
    validate_programs(&config);
    validate_jars(&config);

    println!("cargo:warning=machine.toml validated successfully");
}

/// Format error message lines with box drawing
fn format_error_lines(msg: &str) -> String {
    msg.lines()
        .map(|line| {
            let truncated = if line.len() > 64 {
                format!("{}...", &line[..61])
            } else {
                line.to_string()
            };
            format!("║  {:<64} ║", truncated)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Validate that required sections exist
fn validate_required_sections(config: &toml::Value) {
    let mut errors = Vec::new();

    // Determine motor type (defaults to "stepper")
    let motor_type = config
        .get("machine")
        .and_then(|m| m.get("motor_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("stepper");

    // Check for required basket motor based on motor_type
    let has_basket = match motor_type {
        "stepper" => config
            .get("stepper")
            .and_then(|s| s.get("basket"))
            .is_some(),
        "dc" => config
            .get("dc_motor")
            .and_then(|s| s.get("basket"))
            .is_some(),
        "ac" => config
            .get("ac_motor")
            .and_then(|s| s.get("basket"))
            .is_some(),
        _ => {
            errors.push(format!(
                "Invalid motor_type '{}' - must be 'stepper', 'dc', or 'ac'",
                motor_type
            ));
            true // Skip basket check for invalid motor_type
        }
    };

    if !has_basket {
        let section_name = match motor_type {
            "dc" => "[dc_motor.basket]",
            "ac" => "[ac_motor.basket]",
            _ => "[stepper.basket]",
        };
        errors.push(format!(
            "Missing {} - basket motor is required",
            section_name
        ));
    }

    // Check for at least one profile
    if config.get("profile").is_none() {
        errors.push("Missing [profile.*] section - at least one profile is required".to_string());
    }

    // Check for at least one program
    if config.get("program").is_none() {
        errors.push("Missing [program.*] section - at least one program is required".to_string());
    }

    // Check for at least one jar
    if config.get("jar").is_none() {
        errors.push("Missing [jar.*] section - at least one jar is required".to_string());
    }

    if !errors.is_empty() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: Missing required sections in machine.toml                ║\n\
            ╠══════════════════════════════════════════════════════════════════╣\n\
            {}\n\
            ╚══════════════════════════════════════════════════════════════════╝\n",
            errors
                .iter()
                .map(|e| format!("║  • {:<62} ║", e))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Validate [machine] section
fn validate_machine_section(config: &toml::Value) {
    let machine = match config.get("machine") {
        Some(toml::Value::Table(t)) => t,
        _ => return,
    };

    let mut errors = Vec::new();

    // Validate safe_z against z stepper limits (if z stepper is configured)
    if let Some(toml::Value::Integer(safe_z)) = machine.get("safe_z") {
        // Get z stepper position limits
        if let Some(z_stepper) = config
            .get("stepper")
            .and_then(|s| s.get("z"))
            .and_then(|s| s.as_table())
        {
            let z_min = z_stepper
                .get("position_min")
                .and_then(|v| v.as_integer())
                .unwrap_or(0);

            if let Some(z_max) = z_stepper.get("position_max").and_then(|v| v.as_integer()) {
                if *safe_z < z_min {
                    errors.push(format!(
                        "safe_z ({}) is below stepper.z position_min ({})",
                        safe_z, z_min
                    ));
                }
                if *safe_z > z_max {
                    errors.push(format!(
                        "safe_z ({}) is above stepper.z position_max ({})",
                        safe_z, z_max
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: Invalid machine configuration                            ║\n\
            ╠══════════════════════════════════════════════════════════════════╣\n\
            {}\n\
            ╚══════════════════════════════════════════════════════════════════╝\n",
            errors
                .iter()
                .map(|e| format!("║  • {:<62} ║", e))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Validate profile configurations
fn validate_profiles(config: &toml::Value) {
    let profiles = match config.get("profile") {
        Some(toml::Value::Table(t)) => t,
        _ => return,
    };

    let mut errors = Vec::new();

    for (name, profile) in profiles {
        let profile = match profile {
            toml::Value::Table(t) => t,
            _ => {
                errors.push(format!("[profile.{}] must be a table", name));
                continue;
            }
        };

        // Required fields
        if profile.get("label").is_none() {
            errors.push(format!("[profile.{}] missing 'label'", name));
        }
        if profile.get("rpm").is_none() {
            errors.push(format!("[profile.{}] missing 'rpm'", name));
        }
        if profile.get("time_s").is_none() {
            errors.push(format!("[profile.{}] missing 'time_s'", name));
        }
        if profile.get("direction").is_none() {
            errors.push(format!("[profile.{}] missing 'direction'", name));
        }

        // Validate direction value
        if let Some(toml::Value::String(dir)) = profile.get("direction") {
            if !["cw", "ccw", "alternate"].contains(&dir.as_str()) {
                errors.push(format!(
                    "[profile.{}] direction must be 'cw', 'ccw', or 'alternate'",
                    name
                ));
            }
        }

        // Validate numeric ranges
        if let Some(toml::Value::Integer(rpm)) = profile.get("rpm") {
            if *rpm < 0 || *rpm > 1000 {
                errors.push(format!("[profile.{}] rpm must be 0-1000", name));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: Invalid profile configuration                            ║\n\
            ╠══════════════════════════════════════════════════════════════════╣\n\
            {}\n\
            ╚══════════════════════════════════════════════════════════════════╝\n",
            errors
                .iter()
                .map(|e| format!("║  • {:<62} ║", e))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Validate program configurations
fn validate_programs(config: &toml::Value) {
    let programs = match config.get("program") {
        Some(toml::Value::Table(t)) => t,
        _ => return,
    };

    let jars: Vec<String> = config
        .get("jar")
        .and_then(|j| j.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    let profiles: Vec<String> = config
        .get("profile")
        .and_then(|p| p.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    let mut errors = Vec::new();

    for (name, program) in programs {
        let program = match program {
            toml::Value::Table(t) => t,
            _ => {
                errors.push(format!("[program.{}] must be a table", name));
                continue;
            }
        };

        // Required fields
        if program.get("label").is_none() {
            errors.push(format!("[program.{}] missing 'label'", name));
        }

        // Validate steps
        match program.get("steps") {
            Some(toml::Value::Array(steps)) => {
                if steps.is_empty() {
                    errors.push(format!("[program.{}] steps cannot be empty", name));
                }

                for (i, step) in steps.iter().enumerate() {
                    let step = match step.as_table() {
                        Some(t) => t,
                        None => {
                            errors.push(format!("[program.{}] step {} must be a table", name, i));
                            continue;
                        }
                    };

                    // Check jar reference
                    if let Some(toml::Value::String(jar)) = step.get("jar") {
                        if !jars.contains(jar) {
                            errors.push(format!(
                                "[program.{}] step {} references unknown jar '{}'",
                                name, i, jar
                            ));
                        }
                    } else {
                        errors.push(format!("[program.{}] step {} missing 'jar'", name, i));
                    }

                    // Check profile reference
                    if let Some(toml::Value::String(profile)) = step.get("profile") {
                        if !profiles.contains(profile) {
                            errors.push(format!(
                                "[program.{}] step {} references unknown profile '{}'",
                                name, i, profile
                            ));
                        }
                    } else {
                        errors.push(format!("[program.{}] step {} missing 'profile'", name, i));
                    }
                }
            }
            Some(_) => {
                errors.push(format!("[program.{}] steps must be an array", name));
            }
            None => {
                errors.push(format!("[program.{}] missing 'steps'", name));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: Invalid program configuration                            ║\n\
            ╠══════════════════════════════════════════════════════════════════╣\n\
            {}\n\
            ╚══════════════════════════════════════════════════════════════════╝\n",
            errors
                .iter()
                .map(|e| format!("║  • {:<62} ║", e))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Position limits from a stepper configuration (Klipper-style)
/// Only steppers support position control - DC/AC motors can only detect endstops.
struct PositionLimits {
    min: i64,
    max: i64,
}

/// Get position limits from a stepper configuration
/// Returns None if:
/// - No stepper with that name exists
/// - Stepper exists but has no position_max (not position-controlled)
fn get_stepper_position_limits(config: &toml::Value, stepper_name: &str) -> Option<PositionLimits> {
    let stepper = config
        .get("stepper")
        .and_then(|s| s.get(stepper_name))
        .and_then(|s| s.as_table())?;

    let min = stepper
        .get("position_min")
        .and_then(|v| v.as_integer())
        .unwrap_or(0);
    let max = stepper.get("position_max").and_then(|v| v.as_integer())?;

    Some(PositionLimits { min, max })
}

/// Validate jar configurations
fn validate_jars(config: &toml::Value) {
    let jars = match config.get("jar") {
        Some(toml::Value::Table(t)) => t,
        _ => return,
    };

    let heaters: Vec<String> = config
        .get("heater")
        .and_then(|h| h.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    // Get position limits from stepper configurations (Klipper-style)
    // Only steppers support position control for automated jar movement
    let x_limits = get_stepper_position_limits(config, "x");
    let z_limits = get_stepper_position_limits(config, "z");

    let mut errors = Vec::new();

    for (name, jar) in jars {
        let jar = match jar {
            toml::Value::Table(t) => t,
            _ => {
                errors.push(format!("[jar.{}] must be a table", name));
                continue;
            }
        };

        // x_pos and z_pos are optional (for manual machines)
        // but if present, must be valid numbers (in mm)
        // Also accept legacy names tower_pos/lift_pos for backwards compatibility
        let x_pos = jar.get("x_pos").or_else(|| jar.get("tower_pos"));
        if let Some(toml::Value::Integer(pos)) = x_pos {
            // Validate against x motor's position limits if configured
            if let Some(ref limits) = x_limits {
                if *pos < limits.min || *pos > limits.max {
                    errors.push(format!(
                        "[jar.{}] x_pos {} outside stepper.x range ({}-{})",
                        name, pos, limits.min, limits.max
                    ));
                }
            }
            // Basic sanity check if no motor configured
            if x_limits.is_none() && (*pos < 0 || *pos > 100000) {
                errors.push(format!("[jar.{}] x_pos {} seems unreasonable", name, pos));
            }
        }

        let z_pos = jar.get("z_pos").or_else(|| jar.get("lift_pos"));
        if let Some(toml::Value::Integer(pos)) = z_pos {
            // Validate against z motor's position limits if configured
            if let Some(ref limits) = z_limits {
                if *pos < limits.min || *pos > limits.max {
                    errors.push(format!(
                        "[jar.{}] z_pos {} outside stepper.z range ({}-{})",
                        name, pos, limits.min, limits.max
                    ));
                }
            }
            // Basic sanity check if no motor configured
            if z_limits.is_none() && (*pos < 0 || *pos > 10000) {
                errors.push(format!("[jar.{}] z_pos {} seems unreasonable", name, pos));
            }
        }

        // Validate heater reference if present
        if let Some(toml::Value::String(heater)) = jar.get("heater") {
            if !heaters.contains(heater) {
                errors.push(format!(
                    "[jar.{}] references unknown heater '{}'",
                    name, heater
                ));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "\n\
            ╔══════════════════════════════════════════════════════════════════╗\n\
            ║  ERROR: Invalid jar configuration                                ║\n\
            ╠══════════════════════════════════════════════════════════════════╣\n\
            {}\n\
            ╚══════════════════════════════════════════════════════════════════╝\n",
            errors
                .iter()
                .map(|e| format!("║  • {:<62} ║", e))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
