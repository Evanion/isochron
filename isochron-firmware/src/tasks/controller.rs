//! Main controller task
//!
//! Coordinates state machine, scheduler, and safety monitoring.
//! Receives input events and tick signals, updates motor/heater commands,
//! and triggers display updates.

use defmt::*;
use embassy_futures::select::{select3, Either3};

use isochron_core::config::{JarConfig, MachineCapabilities, ProfileConfig, ProgramConfig};
use isochron_core::state::State;

use crate::channels::{
    EVENT_CHANNEL, HEARTBEAT_RECEIVED, HEATER_CMD, INPUT_CHANNEL, MOTOR_CMD, MOTOR_STALL,
    SCREEN_UPDATE, TEMP_READING,
};
use crate::controller::Controller;
use crate::display::Renderer;
use crate::tasks::display_tx::SCREEN_BUFFER;
use crate::tasks::tick::TICK_SIGNAL;

/// Controller task - main coordination loop
#[embassy_executor::task]
pub async fn controller_task(
    capabilities: MachineCapabilities,
    programs: &'static [ProgramConfig],
    profiles: &'static [ProfileConfig],
    jars: &'static [JarConfig],
) {
    info!("Controller task started");

    // Initialize controller
    let mut controller = Controller::new(capabilities);
    controller.load_config(programs, profiles, jars);

    // Initialize renderer for building screens
    let mut renderer = Renderer::new();

    // Render boot screen
    renderer.render_boot();
    update_screen_buffer(&renderer).await;

    // Complete boot sequence
    controller.boot_complete();
    info!("Boot complete, entering idle state");

    // Render initial menu
    render_current_state(&controller, &mut renderer).await;

    loop {
        // Wait for either: input event, tick, or safety sensor update
        match select3(
            INPUT_CHANNEL.receive(),
            async { TICK_SIGNAL.wait().await },
            async {
                // Poll for safety-related signals (non-blocking check every 100ms)
                embassy_time::Timer::after_millis(100).await
            },
        )
        .await
        {
            Either3::First(input) => {
                // Process input event
                debug!("Input: {:?}", input);
                if let Some(event) = controller.process_input(input) {
                    debug!("Event: {:?}", event);
                    // Log event for debugging
                    let _ = EVENT_CHANNEL.try_send(event);
                }

                // Update motor/heater commands
                MOTOR_CMD.signal(controller.motor_command());
                HEATER_CMD.signal(controller.heater_command());

                // Re-render display
                render_current_state(&controller, &mut renderer).await;
            }

            Either3::Second(now_ms) => {
                // Check for temperature updates from heater task
                if let Some(temp) = TEMP_READING.try_take() {
                    controller.update_temperature(temp);
                }

                // Check for motor stall updates from TMC task
                if let Some(stalled) = MOTOR_STALL.try_take() {
                    controller.update_motor_stall(stalled);
                }

                // Check for heartbeat from display
                if HEARTBEAT_RECEIVED.signaled() {
                    HEARTBEAT_RECEIVED.reset();
                    controller.heartbeat_received();
                    trace!("Heartbeat received, safety updated");
                }

                // Periodic tick - update scheduler and safety
                if let Some(event) = controller.tick(now_ms) {
                    debug!("Tick event: {:?}", event);
                    let _ = EVENT_CHANNEL.try_send(event);

                    // Update motor/heater commands
                    MOTOR_CMD.signal(controller.motor_command());
                    HEATER_CMD.signal(controller.heater_command());

                    // Re-render display for state changes
                    render_current_state(&controller, &mut renderer).await;
                }

                // Periodic display refresh for running state (progress bar, time)
                if controller.state().motor_allowed() {
                    render_current_state(&controller, &mut renderer).await;
                }
            }

            Either3::Third(_) => {
                // Periodic safety signal polling (every 100ms)
                // Check for temperature updates from heater task
                if let Some(temp) = TEMP_READING.try_take() {
                    controller.update_temperature(temp);
                }

                // Check for motor stall updates from TMC task
                if let Some(stalled) = MOTOR_STALL.try_take() {
                    controller.update_motor_stall(stalled);
                }

                // Check for heartbeat from display
                if HEARTBEAT_RECEIVED.signaled() {
                    HEARTBEAT_RECEIVED.reset();
                    controller.heartbeat_received();
                }
            }
        }
    }
}

/// Render the current state to the screen buffer
async fn render_current_state(controller: &Controller, renderer: &mut Renderer) {
    match controller.state() {
        State::Boot => {
            renderer.render_boot();
        }
        State::Idle => {
            // Collect program labels
            let labels: heapless::Vec<&str, 8> = controller.program_labels().take(8).collect();
            renderer.render_menu(&labels, controller.selected_program() as usize);
        }
        State::ProgramSelected => {
            if let Some(program) = controller.get_program(controller.selected_program()) {
                // Build step descriptions
                let mut steps: heapless::Vec<&str, 8> = heapless::Vec::new();
                for step in program.steps.iter().take(5) {
                    let _ = steps.push(step.jar.as_str());
                }

                // Calculate total time (simplified)
                let total_time = controller.step_total_s();

                renderer.render_program_detail(program.label.as_str(), &steps, total_time);
            }
        }
        State::Running => {
            if let (Some(profile), Some(jar)) =
                (controller.current_profile(), controller.current_jar())
            {
                let temp = controller.current_temp_c();
                let target = profile.temperature_c;

                renderer.render_running(
                    controller
                        .get_program(controller.selected_program())
                        .map(|p| p.label.as_str())
                        .unwrap_or(""),
                    controller.current_step_num(),
                    controller.total_steps(),
                    jar.name.as_str(),
                    profile.label.as_str(),
                    controller.motor_command().rpm,
                    controller.step_elapsed_s(),
                    controller.step_total_s(),
                    temp,
                    target,
                );
            }
        }
        State::Paused => {
            let program_name = controller
                .get_program(controller.selected_program())
                .map(|p| p.label.as_str())
                .unwrap_or("");
            renderer.render_paused(
                program_name,
                controller.current_step_num(),
                controller.total_steps(),
            );
        }
        State::SpinOff => {
            // Show spin-off in progress (similar to running but different message)
            if let Some(jar) = controller.current_jar() {
                renderer.render_awaiting_jar(jar.name.as_str(), "Spin-off in progress");
            }
        }
        State::AwaitingJar => {
            if let Some(jar) = controller.current_jar() {
                renderer.render_awaiting_jar(jar.name.as_str(), "Move basket to:");
            }
        }
        State::AwaitingSpinOff => {
            renderer.render_awaiting_jar("", "Lift basket for spin-off");
        }
        State::StepComplete => {
            if let Some(jar) = controller.current_jar() {
                renderer.render_step_complete(jar.name.as_str());
            }
        }
        State::ProgramComplete => {
            let program_name = controller
                .get_program(controller.selected_program())
                .map(|p| p.label.as_str())
                .unwrap_or("");
            renderer.render_complete(program_name, controller.step_elapsed_s());
        }
        State::Error(kind) => {
            let error_type = match kind {
                isochron_core::state::ErrorKind::ThermistorFault => "SENSOR FAULT",
                isochron_core::state::ErrorKind::OverTemperature => "OVER TEMP",
                isochron_core::state::ErrorKind::MotorStall => "MOTOR STALL",
                isochron_core::state::ErrorKind::LinkLost => "LINK LOST",
                isochron_core::state::ErrorKind::ConfigError => "CONFIG ERROR",
                isochron_core::state::ErrorKind::Unknown => "UNKNOWN ERROR",
            };
            renderer.render_error(error_type, "Power cycle to restart");
        }
        State::EditProgram => {
            // Placeholder for edit mode
        }
    }

    update_screen_buffer(renderer).await;
}

/// Copy rendered screen to shared buffer and signal update
async fn update_screen_buffer(renderer: &Renderer) {
    let mut buffer = SCREEN_BUFFER.lock().await;

    // Copy screen content
    buffer.clear();
    for row in 0..8 {
        buffer.set_line(row, renderer.screen().get_line(row));
    }
    if let Some(sel) = renderer.screen().selected_row() {
        buffer.set_selection(sel, renderer.screen().invert_selection());
    }

    // Signal TX task to send update
    SCREEN_UPDATE.signal(());
}
