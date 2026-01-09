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
    AutotuneCommand, AutotuneStatus, CalibrationSaveRequest, AUTOTUNE_CMD, AUTOTUNE_STATUS,
    CALIBRATION_SAVE, EVENT_CHANNEL, HEARTBEAT_RECEIVED, HEATER_CMD, HOMING_CMD, INPUT_CHANNEL,
    MOTOR_CMD, MOTOR_STALL, POSITION_STATUS, SCREEN_UPDATE, TEMP_READING, X_POSITION_CMD,
    Z_POSITION_CMD,
};
use crate::controller::{Controller, HomingState};
use isochron_core::motion::HomingCommand;
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

    // Complete boot sequence - may trigger homing for automated machines
    if let Some(_event) = controller.boot_complete() {
        // Automated machine - start homing sequence
        if let Some(axis) = controller.start_homing() {
            info!("Starting homing sequence with {:?} axis", axis);
            match axis {
                isochron_core::motion::Axis::Z => {
                    HOMING_CMD.signal(HomingCommand::HomeZ);
                }
                isochron_core::motion::Axis::X => {
                    HOMING_CMD.signal(HomingCommand::HomeX);
                }
            }
        }
        info!("Boot complete, homing in progress");
    } else {
        info!("Boot complete, entering idle state");
    }

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

                    // Handle autotune start/cancel
                    use crate::controller::AutotunePhase;
                    use isochron_core::state::Event;
                    match event {
                        Event::StartAutotune => {
                            // Only send command when actually starting (Running phase)
                            if controller.autotune_phase() == AutotunePhase::Running {
                                info!("Starting autotune");
                                AUTOTUNE_CMD.signal(AutotuneCommand::Start {
                                    target_x10: controller.autotune_target_x10(),
                                });
                            }
                        }
                        Event::CancelAutotune => {
                            // Only send cancel if autotune was actually running
                            if controller.autotune_phase() == AutotunePhase::Failed {
                                info!("Canceling autotune");
                                AUTOTUNE_CMD.signal(AutotuneCommand::Cancel);
                            }
                        }
                        _ => {}
                    }
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

                // Check for position status updates from stepper tasks
                while let Ok(status) = POSITION_STATUS.try_receive() {
                    debug!("Position status: {:?}", status);

                    // Get current state before handling
                    let was_lifting = controller.state() == State::Lifting;
                    let was_moving_to_jar = controller.state() == State::MovingToJar;
                    let was_lowering = controller.state() == State::Lowering;

                    if let Some(event) = controller.handle_position_status(status) {
                        debug!("Position event: {:?}", event);
                        let _ = EVENT_CHANNEL.try_send(event);

                        // Check if we need to continue homing sequence (Z done, X next)
                        if controller.homing_state() == HomingState::HomingX {
                            info!("Z homing complete, starting X homing");
                            HOMING_CMD.signal(HomingCommand::HomeX);
                        }

                        // Handle jar transition sequence
                        match event {
                            isochron_core::state::Event::LiftComplete if was_lifting => {
                                // Lift done - move X or prompt user
                                if let Some((next_event, x_pos)) = controller.handle_lift_complete()
                                {
                                    debug!("Lift complete, next: {:?}", next_event);
                                    if matches!(
                                        next_event,
                                        isochron_core::state::Event::StartMoveX
                                    ) {
                                        info!("Starting X move to {} mm", x_pos);
                                        X_POSITION_CMD.signal(x_pos);
                                    }
                                    // PromptNextJar is handled by display - user will click
                                }
                            }
                            isochron_core::state::Event::MoveXComplete if was_moving_to_jar => {
                                // X move done - start lowering
                                if let Some(z_pos) = controller.handle_move_x_complete() {
                                    info!("X move complete, lowering to {} mm", z_pos);
                                    Z_POSITION_CMD.signal(z_pos);
                                }
                            }
                            isochron_core::state::Event::LowerComplete if was_lowering => {
                                // Lower done - resume running
                                controller.handle_lower_complete();
                                info!("Lower complete, resuming");
                            }
                            _ => {}
                        }

                        // Update motor/heater commands
                        MOTOR_CMD.signal(controller.motor_command());
                        HEATER_CMD.signal(controller.heater_command());

                        // Re-render display
                        render_current_state(&controller, &mut renderer).await;
                    }
                }

                // Check for autotune status updates
                if let Some(status) = AUTOTUNE_STATUS.try_take() {
                    use crate::channels::AutotuneFailure;
                    use crate::controller::AutotuneFailureReason;

                    match status {
                        AutotuneStatus::Started => {
                            info!("Autotune started");
                        }
                        AutotuneStatus::Progress { peaks, ticks } => {
                            debug!("Autotune progress: {} peaks, {} ticks", peaks, ticks);
                            controller.update_autotune_progress(peaks, ticks);
                        }
                        AutotuneStatus::Complete {
                            kp_x100,
                            ki_x100,
                            kd_x100,
                        } => {
                            info!(
                                "Autotune complete: Kp={}.{:02}, Ki={}.{:02}, Kd={}.{:02}",
                                kp_x100 / 100,
                                (kp_x100 % 100).abs(),
                                ki_x100 / 100,
                                (ki_x100 % 100).abs(),
                                kd_x100 / 100,
                                (kd_x100 % 100).abs(),
                            );
                            // Store result for display and set phase to Complete
                            controller.set_autotune_complete(kp_x100, ki_x100, kd_x100);
                            // Request calibration save to flash
                            CALIBRATION_SAVE.signal(CalibrationSaveRequest {
                                heater_index: 0,
                                kp_x100,
                                ki_x100,
                                kd_x100,
                            });
                        }
                        AutotuneStatus::Failed(reason) => {
                            warn!("Autotune failed: {:?}", reason);
                            // Convert channel failure type to controller failure type
                            let failure_reason = match reason {
                                AutotuneFailure::OverTemp => AutotuneFailureReason::OverTemp,
                                AutotuneFailure::Timeout => AutotuneFailureReason::Timeout,
                                AutotuneFailure::SensorFault => AutotuneFailureReason::SensorFault,
                                AutotuneFailure::NoOscillation => {
                                    AutotuneFailureReason::NoOscillation
                                }
                                AutotuneFailure::Cancelled => AutotuneFailureReason::Cancelled,
                            };
                            controller.set_autotune_failed(failure_reason);
                        }
                    }
                    // Re-render display for autotune status changes
                    render_current_state(&controller, &mut renderer).await;
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
            // Collect program labels plus autotune option
            let mut labels: heapless::Vec<&str, 8> = controller.program_labels().take(7).collect();
            let _ = labels.push("Autotune Heater");

            // Determine selected index for display
            let selected = if controller.is_autotune_selected() {
                labels.len() - 1 // Last item (autotune)
            } else {
                controller.selected_program() as usize
            };
            renderer.render_menu(&labels, selected);
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
                isochron_core::state::ErrorKind::HomingFailed => "HOMING FAILED",
                isochron_core::state::ErrorKind::PositionOutOfBounds => "POS OUT OF BOUNDS",
                isochron_core::state::ErrorKind::Unknown => "UNKNOWN ERROR",
            };
            renderer.render_error(error_type, "Power cycle to restart");
        }
        State::Homing => {
            // Show which axis is being homed
            let axis = match controller.homing_state() {
                HomingState::HomingZ => "Z",
                HomingState::HomingX => "X",
                HomingState::Idle => "?", // Shouldn't happen in Homing state
            };
            renderer.render_homing(axis);
        }
        State::Lifting => {
            renderer.render_lifting();
        }
        State::MovingToJar => {
            if let Some(jar) = controller.current_jar() {
                renderer.render_moving_to_jar(jar.name.as_str());
            }
        }
        State::Lowering => {
            if let Some(jar) = controller.current_jar() {
                renderer.render_lowering(jar.name.as_str());
            }
        }
        State::EditProgram => {
            // Placeholder for edit mode
        }
        State::Autotuning => {
            use crate::controller::AutotunePhase;
            match controller.autotune_phase() {
                AutotunePhase::Confirming => {
                    // Show confirmation screen
                    renderer.render_autotune_confirm(controller.autotune_target_c());
                }
                AutotunePhase::Running => {
                    // Show progress screen
                    let (peaks, ticks) = controller.autotune_progress();
                    // Convert ticks to seconds (500ms per tick)
                    let elapsed_s = (ticks / 2) as u32;
                    let temp_c = controller.current_temp_c().unwrap_or(0);
                    let target_c = controller.autotune_target_c();
                    renderer.render_autotune_progress(peaks, elapsed_s, temp_c, target_c);
                }
                AutotunePhase::Complete => {
                    // Show result screen
                    if let Some((kp, ki, kd)) = controller.autotune_result() {
                        renderer.render_autotune_complete(kp, ki, kd);
                    }
                }
                AutotunePhase::Failed => {
                    // Show failure screen
                    let reason = controller
                        .autotune_failure()
                        .map(|r| r.as_str())
                        .unwrap_or("Unknown error");
                    renderer.render_autotune_failed(reason);
                }
            }
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
