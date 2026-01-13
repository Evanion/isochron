//! Isochron - Watch Cleaning Machine Firmware
//!
//! Main firmware binary for RP2040-based watch cleaning machines.
//! Implements a Klipper-inspired, config-driven architecture.
//!
//! Named after the Greek "isochronous" meaning "equal time" -
//! reflecting the precision timing of watch movements.

#![no_std]
#![no_main]

extern crate alloc;

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::adc::{Adc, Channel, InterruptHandler as AdcInterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::{DMA_CH2, FLASH, PIO0, PIO1, UART0, UART1};
use embassy_rp::pio::Pio;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_rp::uart::{
    BufferedInterruptHandler, Config as UartConfig, InterruptHandler as UartInterruptHandler, Uart,
};
use embassy_rp::Peri;
use embedded_alloc::LlffHeap as Heap;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use isochron_hal_rp2040::flash::FlashStorage;
use isochron_hal_rp2040::pio::StepGeneratorConfig;
use isochron_hal_rp2040::position_stepper::{PositionStepper, PositionStepperConfig};
use isochron_hal_rp2040::stepper::PioStepper;

use crate::config::{parse_config, ConfigPersistence};

use isochron_core::config::{
    JarConfig, MachineCapabilities, MachineConfig, MotorType, ProfileConfig, ProgramConfig,
    ProgramStep,
};
use isochron_core::scheduler::DirectionMode;

// Heap allocator for TOML parsing
#[global_allocator]
static HEAP: Heap = Heap::empty();

// Heap size: 32KB
const HEAP_SIZE: usize = 32 * 1024;

/// Embedded default configuration (compiled into firmware)
/// Edit machine.toml and rebuild to customize
const EMBEDDED_CONFIG: &str = include_str!("../machine.toml");

mod boards;
mod channels;
mod components;
mod config;
mod controller;
mod display;
mod tasks;

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
    UART1_IRQ => UartInterruptHandler<UART1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO1>;
    ADC_IRQ_FIFO => AdcInterruptHandler;
});

// Static cells for UART buffers (must live forever)
static TX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static RX_BUF: StaticCell<[u8; 256]> = StaticCell::new();

// Static cells for configuration (must live forever for task references)
// Max 8 of each to match MachineConfig limits
static MACHINE_CONFIG: StaticCell<MachineConfig> = StaticCell::new();
static PROGRAMS: StaticCell<[ProgramConfig; 8]> = StaticCell::new();
static PROFILES: StaticCell<[ProfileConfig; 8]> = StaticCell::new();
static JARS: StaticCell<[JarConfig; 8]> = StaticCell::new();

/// Main entry point
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Isochron firmware starting...");

    // Initialize heap allocator
    init_heap();

    // Initialize RP2040 peripherals
    let p = embassy_rp::init(Default::default());
    info!("Peripherals initialized");

    // Load configuration from flash (or use embedded defaults)
    // Also load calibration data and get flash storage back for persistence
    let (config, calibration, flash_storage) = load_config_from_flash(p.FLASH, p.DMA_CH2).await;

    // Get motor type before extracting other config
    let motor_type = config.motor_type;
    info!("Motor type: {:?}", motor_type);

    // Extract motor config values based on motor type
    // Stepper config (only used if motor_type == Stepper)
    let stepper_config_values =
        if motor_type == MotorType::Stepper {
            config.find_stepper("basket").map(|stepper| {
                let full_steps = stepper.full_steps_per_rotation as u32;
                let microsteps = stepper.microsteps as u32;
                let gear_num = stepper.gear_ratio_num as u32;
                let gear_den = stepper.gear_ratio_den.max(1) as u32;
                let steps = full_steps * microsteps * gear_num / gear_den;
                info!(
                "Stepper config: {} steps/rev ({}x{} * {}/{}), pins={}/{}/{}, enable_inverted={}",
                steps, full_steps, microsteps, gear_num, gear_den,
                stepper.step_pin.pin, stepper.dir_pin.pin, stepper.enable_pin.pin,
                stepper.enable_pin.inverted
            );
                (
                    steps,
                    stepper.enable_pin.inverted,
                    stepper.microsteps,
                    stepper.step_pin.pin,
                    stepper.dir_pin.pin,
                    stepper.enable_pin.pin,
                )
            })
        } else {
            None
        };

    // DC motor config (only used if motor_type == Dc)
    let dc_motor_config_values = if motor_type == MotorType::Dc {
        config.find_dc_motor("basket").map(|dc| {
            info!(
                "DC motor config: pwm_freq={}Hz, min_duty={}%, soft_start={}ms",
                dc.pwm_frequency, dc.min_duty, dc.soft_start_ms
            );
            (dc.min_duty, dc.soft_start_ms, dc.soft_stop_ms)
        })
    } else {
        None
    };

    // AC motor config (only used if motor_type == Ac)
    let ac_motor_config_values = if motor_type == MotorType::Ac {
        config.find_ac_motor("basket").map(|ac| {
            info!("AC motor config: active_high={}", ac.active_high);
            (ac.active_high, ac.direction_pin.is_some())
        })
    } else {
        None
    };

    // Extract TMC2209 config values (only for stepper)
    let tmc_config_values = if motor_type == MotorType::Stepper {
        config
            .tmc2209s
            .iter()
            .find(|t| t.stepper_name.as_str() == "basket")
            .map(|tmc| {
                info!(
                    "TMC2209 config: addr={}, run={}mA, stealthchop={}, sg={}",
                    tmc.uart_address, tmc.run_current_ma, tmc.stealthchop, tmc.stall_threshold
                );
                (
                    tmc.uart_address,
                    tmc.run_current_ma,
                    tmc.hold_current_ma,
                    tmc.stealthchop,
                    tmc.stall_threshold,
                )
            })
    } else {
        None
    };

    // Extract heater config values including PID coefficients
    let heater_config_values = config.find_heater("dryer").map(|heater| {
        info!(
            "Heater config: max_temp={}°C, hysteresis={}°C, control={:?}",
            heater.max_temp, heater.hysteresis, heater.control
        );
        if heater.pid_kp_x100.is_some()
            || heater.pid_ki_x100.is_some()
            || heater.pid_kd_x100.is_some()
        {
            info!(
                "  PID from TOML: Kp={:?}, Ki={:?}, Kd={:?}",
                heater.pid_kp_x100, heater.pid_ki_x100, heater.pid_kd_x100
            );
        }
        (
            heater.max_temp,
            heater.hysteresis,
            heater.control,
            heater.pid_kp_x100,
            heater.pid_ki_x100,
            heater.pid_kd_x100,
        )
    });

    // Detect machine capabilities before moving config
    let has_z = config.find_stepper("z").is_some();
    let has_x = config.find_stepper("x").is_some();
    let has_lid = config.find_stepper("lid").is_some();
    let heater_count = config.heater_hw.len() as u8;
    let safe_z = config.safe_z.unwrap_or_else(|| {
        // Default to Z motor's position_min (top of travel) if not specified
        config
            .find_stepper("z")
            .map(|s| s.position_min)
            .unwrap_or(0)
    });

    info!(
        "Machine capabilities: has_z={}, has_x={}, has_lid={}, heaters={}, safe_z={}",
        has_z, has_x, has_lid, heater_count, safe_z
    );

    // Extract Z stepper config for position control (before moving config)
    let z_stepper_config = config.find_stepper("z").map(|stepper| {
        let full_steps = stepper.full_steps_per_rotation as u32;
        let microsteps = stepper.microsteps as u32;
        let gear_num = stepper.gear_ratio_num as u32;
        let gear_den = stepper.gear_ratio_den.max(1) as u32;
        let rotation_dist = stepper.rotation_distance as u32;
        // steps_per_mm = (full_steps * microsteps * gear_ratio) / rotation_distance
        let steps_per_mm = (full_steps * microsteps * gear_num) / (gear_den * rotation_dist);
        info!(
            "Z stepper: {} steps/mm, range {}..{} mm, endstop={:?}",
            steps_per_mm,
            stepper.position_min,
            stepper.position_max.unwrap_or(200),
            stepper.endstop_pin.map(|p| p.pin)
        );
        (
            stepper.step_pin.pin,
            stepper.dir_pin.pin,
            stepper.enable_pin.pin,
            stepper.enable_pin.inverted,
            stepper.endstop_pin.map(|p| p.pin).unwrap_or(25), // Default GPIO25 for Z endstop
            steps_per_mm,
            stepper.position_min,
            stepper.position_max.unwrap_or(200),
            stepper.position_endstop.unwrap_or(0),
            stepper.homing_speed.unwrap_or(10),
            stepper.homing_retract_dist.unwrap_or(5),
            stepper.homing_positive_dir.unwrap_or(false),
        )
    });

    // Extract X stepper config for position control (before moving config)
    let x_stepper_config = config.find_stepper("x").map(|stepper| {
        let full_steps = stepper.full_steps_per_rotation as u32;
        let microsteps = stepper.microsteps as u32;
        let gear_num = stepper.gear_ratio_num as u32;
        let gear_den = stepper.gear_ratio_den.max(1) as u32;
        let rotation_dist = stepper.rotation_distance as u32;
        let steps_per_mm = (full_steps * microsteps * gear_num) / (gear_den * rotation_dist);
        info!(
            "X stepper: {} steps/mm, range {}..{} mm, endstop={:?}",
            steps_per_mm,
            stepper.position_min,
            stepper.position_max.unwrap_or(800),
            stepper.endstop_pin.map(|p| p.pin)
        );
        (
            stepper.step_pin.pin,
            stepper.dir_pin.pin,
            stepper.enable_pin.pin,
            stepper.enable_pin.inverted,
            stepper.endstop_pin.map(|p| p.pin).unwrap_or(4), // Default GPIO4 for X endstop
            steps_per_mm,
            stepper.position_min,
            stepper.position_max.unwrap_or(800),
            stepper.position_endstop.unwrap_or(0),
            stepper.homing_speed.unwrap_or(30),
            stepper.homing_retract_dist.unwrap_or(5),
            stepper.homing_positive_dir.unwrap_or(false),
        )
    });

    // Now we can move config
    let (programs, profiles, jars) = init_config_from_machine(config);
    info!("Configuration loaded");

    // Setup UART for display communication
    let uart_config = UartConfig::default(); // 115200 baud default

    let tx_buf = TX_BUF.init([0u8; 256]);
    let rx_buf = RX_BUF.init([0u8; 256]);

    let uart = Uart::new_blocking(p.UART0, p.PIN_0, p.PIN_1, uart_config);
    let uart = uart.into_buffered(Irqs, tx_buf, rx_buf);
    let (tx, rx) = uart.split();

    info!("UART initialized for display communication");

    // Motor hardware initialization (conditional based on motor_type)
    // Only one motor type is active at a time - use enum to hold resources
    //
    // Pin slot allocation (SKR Pico):
    //   - E slot (14, 13, 15): Basket stepper motor
    //   - X slot (11, 10, 12): X position stepper OR DC/AC motor (mutually exclusive)
    //   - Z slot (19, 28, 2):  Z position stepper
    //   - Y slot (6, 5, 7):    Lid stepper (future)
    enum MotorResources {
        Stepper(PioStepper<'static, PIO0, 0>),
        Dc(
            Pwm<'static>,
            Option<Output<'static>>,
            Option<Output<'static>>,
            tasks::DcMotorFwConfig,
        ),
        Ac(
            Output<'static>,
            Option<Output<'static>>,
            tasks::AcMotorFwConfig,
        ),
    }

    // For stepper motor type, X slot is available for X position stepper
    // For DC/AC motor types, X slot is used by the main motor
    let motor_resources = match motor_type {
        MotorType::Stepper => {
            // Setup PIO0 for basket stepper motor control
            // Pin assignments from config (SKR Pico E slot: STEP=GPIO14, DIR=GPIO13, ENABLE=GPIO15)
            let Pio {
                mut common, sm0, ..
            } = Pio::new(p.PIO0, Irqs);

            let (steps_per_rev, enable_inverted, _microsteps, step_pin, dir_pin, enable_pin) =
                stepper_config_values.unwrap_or_else(|| {
                    warn!("No stepper config found, using defaults (E slot: 14/13/15)");
                    (3200, false, 16, 14, 13, 15) // 200 steps * 16 microsteps, E slot pins
                });

            // Validate expected E slot pins (SKR Pico E connector for basket motor)
            if step_pin != 14 || dir_pin != 13 || enable_pin != 15 {
                error!(
                    "Basket stepper pins ({}/{}/{}) don't match SKR Pico E slot (14/13/15)",
                    step_pin, dir_pin, enable_pin
                );
            }

            let stepper_config = StepGeneratorConfig {
                step_pin,
                dir_pin,
                enable_pin,
                enable_inverted,
                steps_per_rev,
            };

            let stepper = PioStepper::new(
                &mut common,
                sm0,
                p.PIN_14, // E slot step pin
                p.PIN_13, // E slot dir pin
                p.PIN_15, // E slot enable pin
                stepper_config,
            );

            info!(
                "PIO stepper initialized on E slot (pins {}/{}/{})",
                step_pin, dir_pin, enable_pin
            );

            // Initialize X position stepper if configured (uses X slot pins)
            // This must happen here because DC/AC motor types use the same pins
            if let Some((
                x_step_pin,
                x_dir_pin,
                x_enable_pin,
                x_enable_inverted,
                x_endstop_pin,
                x_steps_per_mm,
                x_position_min,
                x_position_max,
                x_position_endstop,
                x_homing_speed,
                x_homing_retract,
                x_home_to_max,
            )) = x_stepper_config
            {
                if x_step_pin == 11 && x_dir_pin == 10 && x_enable_pin == 12 && x_endstop_pin == 4 {
                    // Setup PIO1 for position steppers
                    let Pio {
                        common: mut common1,
                        sm0: sm1_0,
                        sm1: sm1_1,
                        ..
                    } = Pio::new(p.PIO1, Irqs);

                    // Z stepper first (if configured)
                    if let Some((
                        z_step_pin,
                        z_dir_pin,
                        z_enable_pin,
                        z_enable_inverted,
                        z_endstop_pin,
                        z_steps_per_mm,
                        z_position_min,
                        z_position_max,
                        z_position_endstop,
                        z_homing_speed,
                        z_homing_retract,
                        z_home_to_max,
                    )) = z_stepper_config
                    {
                        if z_step_pin == 19
                            && z_dir_pin == 28
                            && z_enable_pin == 2
                            && z_endstop_pin == 25
                        {
                            let z_hw_config = StepGeneratorConfig {
                                step_pin: z_step_pin,
                                dir_pin: z_dir_pin,
                                enable_pin: z_enable_pin,
                                enable_inverted: z_enable_inverted,
                                steps_per_rev: z_steps_per_mm * 8,
                            };
                            let z_pos_config = PositionStepperConfig {
                                stepper: z_hw_config,
                                steps_per_mm: z_steps_per_mm,
                                position_min_mm: z_position_min,
                                position_max_mm: z_position_max,
                                position_endstop_mm: z_position_endstop,
                                homing_speed_mm_s: z_homing_speed,
                                homing_retract_mm: z_homing_retract,
                                move_speed_mm_s: 50,
                                endstop_active_low: true,
                                home_to_max: z_home_to_max,
                            };
                            let z_stepper = PositionStepper::new(
                                &mut common1,
                                sm1_0,
                                p.PIN_19,
                                p.PIN_28,
                                p.PIN_2,
                                p.PIN_25,
                                z_pos_config,
                            );
                            spawner.spawn(tasks::z_stepper_task(z_stepper)).unwrap();
                            info!("Z position stepper spawned (pins 19/28/2/25)");
                        }
                    }

                    // X stepper
                    let x_hw_config = StepGeneratorConfig {
                        step_pin: x_step_pin,
                        dir_pin: x_dir_pin,
                        enable_pin: x_enable_pin,
                        enable_inverted: x_enable_inverted,
                        steps_per_rev: x_steps_per_mm * 200,
                    };
                    let x_pos_config = PositionStepperConfig {
                        stepper: x_hw_config,
                        steps_per_mm: x_steps_per_mm,
                        position_min_mm: x_position_min,
                        position_max_mm: x_position_max,
                        position_endstop_mm: x_position_endstop,
                        homing_speed_mm_s: x_homing_speed,
                        homing_retract_mm: x_homing_retract,
                        move_speed_mm_s: 100,
                        endstop_active_low: true,
                        home_to_max: x_home_to_max,
                    };
                    let x_stepper = PositionStepper::new(
                        &mut common1,
                        sm1_1,
                        p.PIN_11,
                        p.PIN_10,
                        p.PIN_12,
                        p.PIN_4,
                        x_pos_config,
                    );
                    spawner.spawn(tasks::x_stepper_task(x_stepper)).unwrap();
                    info!("X position stepper spawned (pins 11/10/12/4)");
                } else {
                    error!(
                        "X stepper pins ({}/{}/{}/{}) don't match X slot (11/10/12/4)",
                        x_step_pin, x_dir_pin, x_enable_pin, x_endstop_pin
                    );
                }
            } else if has_z {
                // Only Z stepper configured (no X)
                let Pio {
                    common: mut common1,
                    sm0: sm1_0,
                    ..
                } = Pio::new(p.PIO1, Irqs);

                if let Some((
                    z_step_pin,
                    z_dir_pin,
                    z_enable_pin,
                    z_enable_inverted,
                    z_endstop_pin,
                    z_steps_per_mm,
                    z_position_min,
                    z_position_max,
                    z_position_endstop,
                    z_homing_speed,
                    z_homing_retract,
                    z_home_to_max,
                )) = z_stepper_config
                {
                    if z_step_pin == 19
                        && z_dir_pin == 28
                        && z_enable_pin == 2
                        && z_endstop_pin == 25
                    {
                        let z_hw_config = StepGeneratorConfig {
                            step_pin: z_step_pin,
                            dir_pin: z_dir_pin,
                            enable_pin: z_enable_pin,
                            enable_inverted: z_enable_inverted,
                            steps_per_rev: z_steps_per_mm * 8,
                        };
                        let z_pos_config = PositionStepperConfig {
                            stepper: z_hw_config,
                            steps_per_mm: z_steps_per_mm,
                            position_min_mm: z_position_min,
                            position_max_mm: z_position_max,
                            position_endstop_mm: z_position_endstop,
                            homing_speed_mm_s: z_homing_speed,
                            homing_retract_mm: z_homing_retract,
                            move_speed_mm_s: 50,
                            endstop_active_low: true,
                            home_to_max: z_home_to_max,
                        };
                        let z_stepper = PositionStepper::new(
                            &mut common1,
                            sm1_0,
                            p.PIN_19,
                            p.PIN_28,
                            p.PIN_2,
                            p.PIN_25,
                            z_pos_config,
                        );
                        spawner.spawn(tasks::z_stepper_task(z_stepper)).unwrap();
                        info!("Z position stepper spawned (pins 19/28/2/25)");
                    }
                }
            }

            MotorResources::Stepper(stepper)
        }
        MotorType::Dc => {
            // PWM on GPIO11 (slice 5, channel B)
            let mut pwm_config = PwmConfig::default();
            pwm_config.top = 1000; // 125kHz / 1000 = 125Hz base
            pwm_config.compare_b = 0; // Start at 0% duty
            let pwm = Pwm::new_output_b(p.PWM_SLICE5, p.PIN_11, pwm_config);

            // Direction pin (GPIO10)
            let dir_pin = Output::new(p.PIN_10, Level::Low);

            // Enable pin (GPIO12)
            let enable_pin = Output::new(p.PIN_12, Level::Low);

            let (min_duty, soft_start_ms, soft_stop_ms) =
                dc_motor_config_values.unwrap_or_else(|| {
                    warn!("No DC motor config found, using defaults");
                    (20, 500, 300)
                });

            let fw_config = tasks::DcMotorFwConfig {
                min_duty,
                soft_start_ms,
                soft_stop_ms,
                pwm_top: 1000,
            };

            info!("DC motor PWM initialized");
            MotorResources::Dc(pwm, Some(dir_pin), Some(enable_pin), fw_config)
        }
        MotorType::Ac => {
            // Relay pin (GPIO12) - same as enable pin on stepper
            let relay_pin = Output::new(p.PIN_12, Level::Low);

            // Direction pin (GPIO10) - optional for reversible AC motors
            let (active_high, has_direction) = ac_motor_config_values.unwrap_or_else(|| {
                warn!("No AC motor config found, using defaults");
                (true, false)
            });

            let dir_pin = if has_direction {
                Some(Output::new(p.PIN_10, Level::Low))
            } else {
                None
            };

            let fw_config = tasks::AcMotorFwConfig {
                relay_type: isochron_drivers::motor::ac::AcRelayType::Mechanical,
                min_switch_delay_ms: 100,
                active_high,
                has_direction,
            };

            info!("AC motor relay initialized");
            MotorResources::Ac(relay_pin, dir_pin, fw_config)
        }
    };

    // Setup ADC for temperature sensing
    // Pin assignment is board-specific (SKR Pico TH0: GPIO27)
    let adc = Adc::new(p.ADC, Irqs, embassy_rp::adc::Config::default());
    let therm_channel = Channel::new_pin(p.PIN_27, embassy_rp::gpio::Pull::None);

    // Setup heater output
    // Pin assignment is board-specific (SKR Pico HE0: GPIO23)
    let heater_pin = Output::new(p.PIN_23, Level::Low);

    // Heater settings from config with calibration fallback
    // Priority: TOML config > Calibration from flash > Defaults
    let heater_config = if let Some((max_temp, hysteresis, control, toml_kp, toml_ki, toml_kd)) =
        heater_config_values
    {
        // Get calibration values for heater 0 (dryer) if available
        let cal = calibration.get(0);
        let (cal_kp, cal_ki, cal_kd) = if let Some(c) = cal {
            info!(
                "Loaded PID calibration from flash: Kp={}.{:02}, Ki={}.{:02}, Kd={}.{:02}",
                c.kp_x100 / 100,
                (c.kp_x100 % 100).abs(),
                c.ki_x100 / 100,
                (c.ki_x100 % 100).abs(),
                c.kd_x100 / 100,
                (c.kd_x100 % 100).abs(),
            );
            (Some(c.kp_x100), Some(c.ki_x100), Some(c.kd_x100))
        } else {
            (None, None, None)
        };

        // TOML values take priority over calibration
        let pid_kp = toml_kp.or(cal_kp).unwrap_or(0);
        let pid_ki = toml_ki.or(cal_ki).unwrap_or(0);
        let pid_kd = toml_kd.or(cal_kd).unwrap_or(0);

        if pid_kp != 0 || pid_ki != 0 || pid_kd != 0 {
            info!(
                "Using PID coefficients: Kp={}.{:02}, Ki={}.{:02}, Kd={}.{:02}",
                pid_kp / 100,
                (pid_kp % 100).abs(),
                pid_ki / 100,
                (pid_ki % 100).abs(),
                pid_kd / 100,
                (pid_kd % 100).abs(),
            );
        }

        tasks::HeaterConfig {
            control_mode: control,
            max_temp_c: max_temp,
            hysteresis_c: hysteresis,
            pullup_ohms: 4700, // Standard 4.7K pullup (could be configurable)
            adc_max: 4096,
            pid_kp_x100: pid_kp,
            pid_ki_x100: pid_ki,
            pid_kd_x100: pid_kd,
            ..Default::default()
        }
    } else {
        warn!("No dryer heater config found, using defaults");
        tasks::HeaterConfig::default()
    };

    info!("ADC and heater initialized");

    // TMC2209 setup (only for stepper motor type)
    let tmc_resources = if motor_type == MotorType::Stepper {
        // Setup UART1 for TMC2209 communication
        // Pin assignments are board-specific (SKR Pico TMC: GPIO8 TX, GPIO9 RX)
        let tmc_uart_config = {
            let mut cfg = UartConfig::default();
            cfg.baudrate = 115200;
            cfg
        };
        let tmc_uart = Uart::new(
            p.UART1,
            p.PIN_8,
            p.PIN_9,
            Irqs,
            p.DMA_CH0,
            p.DMA_CH1,
            tmc_uart_config,
        );
        let (tmc_tx, _tmc_rx) = tmc_uart.split();

        // Get microsteps from stepper config for TMC
        let stepper_microsteps = stepper_config_values
            .map(|(_, _, ms, _, _, _)| ms)
            .unwrap_or(16);

        // TMC2209 configuration from config (already extracted above)
        let tmc_config =
            if let Some((uart_addr, run_ma, hold_ma, stealthchop, sg_thresh)) = tmc_config_values {
                isochron_drivers::stepper::tmc2209::Tmc2209Config {
                    uart_address: uart_addr,
                    run_current_ma: run_ma,
                    hold_current_ma: hold_ma,
                    stealthchop,
                    stallguard_threshold: sg_thresh,
                    microsteps: stepper_microsteps.into(), // u8 -> u16 safely
                }
            } else {
                warn!("No TMC2209 config found, using defaults");
                isochron_drivers::stepper::tmc2209::Tmc2209Config {
                    uart_address: 0,
                    run_current_ma: 800,
                    hold_current_ma: 400,
                    stealthchop: true,
                    stallguard_threshold: 80,
                    microsteps: 16,
                }
            };

        info!("TMC UART initialized");

        // Setup TMC2209 DIAG pin for StallGuard stall detection
        // SKR Pico stepper X DIAG pin is GPIO17
        let diag_pin = Input::new(p.PIN_17, Pull::Down);
        let stall_config = tasks::StallMonitorConfig::default();

        info!("TMC DIAG pin initialized");

        Some((tmc_tx, tmc_config, diag_pin, stall_config))
    } else {
        None
    };

    // Machine capabilities (detected from config)
    let capabilities = MachineCapabilities::from_config(has_z, has_x, has_lid, heater_count);

    // Spawn tasks
    spawner.spawn(tasks::tick_task()).unwrap();
    spawner.spawn(tasks::display_rx_task(rx)).unwrap();
    spawner.spawn(tasks::display_tx_task(tx)).unwrap();

    // Motor task - spawn based on motor resources
    match motor_resources {
        MotorResources::Stepper(stepper) => {
            spawner.spawn(tasks::stepper_task(stepper)).unwrap();
            info!("Stepper motor task spawned");
            // TMC2209 and stall monitor tasks (only for stepper)
            if let Some((tmc_tx, tmc_config, diag_pin, stall_config)) = tmc_resources {
                spawner
                    .spawn(tasks::tmc_init_task(tmc_tx, tmc_config))
                    .unwrap();
                spawner
                    .spawn(tasks::stall_monitor_task(diag_pin, stall_config))
                    .unwrap();
                info!("TMC and stall monitor tasks spawned");
            }
        }
        MotorResources::Dc(pwm, dir_pin, enable_pin, fw_config) => {
            spawner
                .spawn(tasks::dc_motor_task(pwm, dir_pin, enable_pin, fw_config))
                .unwrap();
            info!("DC motor task spawned");
        }
        MotorResources::Ac(relay_pin, dir_pin, fw_config) => {
            spawner
                .spawn(tasks::ac_motor_task(relay_pin, dir_pin, fw_config))
                .unwrap();
            info!("AC motor task spawned");
        }
    }

    spawner
        .spawn(tasks::heater_task(
            adc,
            therm_channel,
            heater_pin,
            heater_config,
        ))
        .unwrap();
    spawner
        .spawn(tasks::calibration_task(flash_storage))
        .unwrap();
    spawner
        .spawn(tasks::controller_task(
            capabilities,
            safe_z,
            programs,
            profiles,
            jars,
        ))
        .unwrap();

    info!("All tasks spawned, firmware running");

    // Main task has nothing else to do - all work happens in spawned tasks
    // We could use this for watchdog or other system monitoring
    loop {
        embassy_time::Timer::after_secs(60).await;
        trace!("Main loop heartbeat");
    }
}

/// Initialize the heap allocator
fn init_heap() {
    use core::mem::MaybeUninit;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    #[allow(static_mut_refs)]
    unsafe {
        HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE)
    }
}

use isochron_core::config::CalibrationData;

/// Load configuration from flash storage
///
/// Attempts to load TOML config from flash. If not found or invalid,
/// returns the embedded default configuration.
/// Also loads PID calibration data and returns the FlashStorage for future saves.
async fn load_config_from_flash(
    flash: Peri<'static, FLASH>,
    dma: Peri<'static, DMA_CH2>,
) -> (MachineConfig, CalibrationData, FlashStorage<'static>) {
    let flash_storage = FlashStorage::new(flash, dma);
    let mut persistence = ConfigPersistence::new(flash_storage);

    let default_config = create_default_config();

    let config = match persistence.load().await {
        Ok(config) => {
            info!("Loaded configuration from flash");
            config
        }
        Err(_) => {
            // Flash empty or invalid - use embedded defaults
            info!("No valid configuration in flash, using embedded defaults");
            default_config
        }
    };

    // Reclaim the storage to load calibration
    let mut storage = persistence.into_storage();

    // Load PID calibration data
    let calibration = crate::config::load_calibration(&mut storage).await;

    (config, calibration, storage)
}

/// Convert MachineConfig to static slices for task consumption
///
/// Copies config data into static cells that live for the program duration.
fn init_config_from_machine(
    config: MachineConfig,
) -> (
    &'static [ProgramConfig],
    &'static [ProfileConfig],
    &'static [JarConfig],
) {
    // Store full config (for potential future use)
    let stored_config = MACHINE_CONFIG.init(config);

    // Copy programs to static array
    let mut programs_arr: [ProgramConfig; 8] = Default::default();
    let program_count = stored_config.programs.len();
    for (i, prog) in stored_config.programs.iter().enumerate() {
        programs_arr[i] = prog.clone();
    }
    let programs = PROGRAMS.init(programs_arr);

    // Copy profiles to static array
    let mut profiles_arr: [ProfileConfig; 8] = Default::default();
    let profile_count = stored_config.profiles.len();
    for (i, prof) in stored_config.profiles.iter().enumerate() {
        profiles_arr[i] = prof.clone();
    }
    let profiles = PROFILES.init(profiles_arr);

    // Copy jars to static array
    let mut jars_arr: [JarConfig; 8] = Default::default();
    let jar_count = stored_config.jars.len();
    for (i, jar) in stored_config.jars.iter().enumerate() {
        jars_arr[i] = jar.clone();
    }
    let jars = JARS.init(jars_arr);

    // Return slices of actual data (not full arrays)
    (
        &programs[..program_count],
        &profiles[..profile_count],
        &jars[..jar_count],
    )
}

/// Create the embedded default configuration
///
/// Parses the machine.toml file that was embedded at compile time.
/// This is used as a fallback when no flash config exists.
fn create_default_config() -> MachineConfig {
    match parse_config(EMBEDDED_CONFIG) {
        Ok(config) => {
            info!("Parsed embedded configuration successfully");
            config
        }
        Err(e) => {
            // This should never happen if machine.toml is valid
            // Fall back to minimal defaults if embedded config is broken
            error!(
                "Failed to parse embedded config: {:?}",
                defmt::Debug2Format(&e)
            );
            error!("Using minimal fallback configuration");
            create_minimal_fallback_config()
        }
    }
}

/// Minimal fallback configuration if embedded TOML parsing fails
///
/// This is a last resort - should only happen during development if
/// machine.toml has syntax errors.
fn create_minimal_fallback_config() -> MachineConfig {
    use heapless::String;

    let mut config = MachineConfig::default();

    // Single profile
    let mut label: String<16> = String::new();
    let _ = label.push_str("Default");
    let profile = ProfileConfig {
        label,
        rpm: 60,
        time_s: 60,
        direction: DirectionMode::Clockwise,
        iterations: 1,
        ..Default::default()
    };
    let _ = config.profiles.push(profile);

    // Single jar
    let mut jar_name: String<16> = String::new();
    let _ = jar_name.push_str("jar1");
    let jar = JarConfig {
        name: jar_name,
        x_pos: 0,
        z_pos: 0,
        ..Default::default()
    };
    let _ = config.jars.push(jar);

    // Single program
    let mut prog_label: String<16> = String::new();
    let _ = prog_label.push_str("Manual");

    let mut j: String<16> = String::new();
    let _ = j.push_str("jar1");
    let mut p: String<16> = String::new();
    let _ = p.push_str("Default");
    let step = ProgramStep { jar: j, profile: p };

    let mut steps = heapless::Vec::new();
    let _ = steps.push(step);

    let program = ProgramConfig {
        label: prog_label,
        steps,
    };
    let _ = config.programs.push(program);

    config
}
