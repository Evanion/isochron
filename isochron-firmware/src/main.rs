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
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};
use embassy_rp::peripherals::{FLASH, PIO0, UART0, UART1};
use embassy_rp::pio::Pio;
use embassy_rp::uart::{BufferedInterruptHandler, Config as UartConfig, InterruptHandler as UartInterruptHandler, Uart};
use embedded_alloc::LlffHeap as Heap;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use isochron_hal_rp2040::flash::FlashStorage;
use isochron_hal_rp2040::pio::StepGeneratorConfig;
use isochron_hal_rp2040::stepper::PioStepper;

use crate::config::{parse_config, ConfigPersistence};

use isochron_core::config::{
    JarConfig, MachineCapabilities, MachineConfig, ProfileConfig, ProgramConfig, ProgramStep,
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
    let config = load_config_from_flash(p.FLASH, p.DMA_CH2).await;

    // Extract stepper config values before moving config
    // (find_stepper returns a reference, so we clone what we need)
    let (steps_per_rev, enable_inverted, stepper_microsteps) = {
        if let Some(stepper) = config.find_stepper("spin") {
            let full_steps = stepper.full_steps_per_rotation as u32;
            let microsteps = stepper.microsteps as u32;
            let gear_num = stepper.gear_ratio_num as u32;
            let gear_den = stepper.gear_ratio_den.max(1) as u32;
            let steps = full_steps * microsteps * gear_num / gear_den;
            info!(
                "Stepper config: {} steps/rev ({}x{} * {}/{}), enable_inverted={}",
                steps, full_steps, microsteps, gear_num, gear_den, stepper.enable_pin.inverted
            );
            (steps, stepper.enable_pin.inverted, stepper.microsteps)
        } else {
            warn!("No spin stepper config found, using defaults");
            (200 * 16 * 3, true, 16u8) // Default: 200 steps * 16 microsteps * 3:1 ratio
        }
    };

    // Extract TMC2209 config values
    let tmc_config_values = config
        .tmc2209s
        .iter()
        .find(|t| t.stepper_name.as_str() == "spin")
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
        });

    // Extract heater config values
    let heater_config_values = config.find_heater("dryer").map(|heater| {
        info!(
            "Heater config: max_temp={}°C, hysteresis={}°C",
            heater.max_temp, heater.hysteresis
        );
        (heater.max_temp, heater.hysteresis)
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

    // Setup PIO0 for stepper motor control
    // Pin assignments are board-specific (SKR Pico: STEP=GPIO11, DIR=GPIO10, ENABLE=GPIO12)
    // Motor parameters come from config (already extracted above)
    let Pio {
        mut common,
        sm0,
        ..
    } = Pio::new(p.PIO0, Irqs);

    let stepper_config = StepGeneratorConfig {
        step_pin: 11,
        dir_pin: 10,
        enable_pin: 12,
        enable_inverted,
        steps_per_rev,
    };

    let stepper = PioStepper::new(
        &mut common,
        sm0,
        p.PIN_11,
        AnyPin::from(p.PIN_10),
        AnyPin::from(p.PIN_12),
        stepper_config,
    );

    info!("PIO stepper initialized");

    // Setup ADC for temperature sensing
    // Pin assignment is board-specific (SKR Pico TH0: GPIO27)
    let adc = Adc::new(p.ADC, Irqs, embassy_rp::adc::Config::default());
    let therm_channel = Channel::new_pin(p.PIN_27, embassy_rp::gpio::Pull::None);

    // Setup heater output
    // Pin assignment is board-specific (SKR Pico HE0: GPIO23)
    let heater_pin = Output::new(p.PIN_23, Level::Low);

    // Heater settings from config (already extracted above)
    let heater_config = if let Some((max_temp, hysteresis)) = heater_config_values {
        tasks::HeaterConfig {
            max_temp_c: max_temp,
            hysteresis_c: hysteresis,
            pullup_ohms: 4700, // Standard 4.7K pullup (could be configurable)
            adc_max: 4096,
        }
    } else {
        warn!("No dryer heater config found, using defaults");
        tasks::HeaterConfig {
            max_temp_c: 55,
            hysteresis_c: 2,
            pullup_ohms: 4700,
            adc_max: 4096,
        }
    };

    info!("ADC and heater initialized");

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

    // TMC2209 configuration from config (already extracted above)
    let tmc_config = if let Some((uart_addr, run_ma, hold_ma, stealthchop, sg_thresh)) =
        tmc_config_values
    {
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

    // Machine capabilities (manual machine for now - no lift/tower motors)
    let capabilities = MachineCapabilities {
        has_lift: false,
        has_tower: false,
        has_lid: false,
        heater_count: 1,
        is_automated: false,
    };

    // Spawn tasks
    spawner.spawn(tasks::tick_task()).unwrap();
    spawner.spawn(tasks::display_rx_task(rx)).unwrap();
    spawner.spawn(tasks::display_tx_task(tx)).unwrap();
    spawner.spawn(tasks::stepper_task(stepper)).unwrap();
    spawner
        .spawn(tasks::heater_task(adc, therm_channel, heater_pin, heater_config))
        .unwrap();
    spawner.spawn(tasks::tmc_init_task(tmc_tx, tmc_config)).unwrap();
    spawner
        .spawn(tasks::stall_monitor_task(diag_pin, stall_config))
        .unwrap();
    spawner
        .spawn(tasks::controller_task(capabilities, programs, profiles, jars))
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

/// Load configuration from flash storage
///
/// Attempts to load TOML config from flash. If not found or invalid,
/// returns the embedded default configuration.
async fn load_config_from_flash(
    flash: FLASH,
    dma: embassy_rp::peripherals::DMA_CH2,
) -> MachineConfig {
    let flash_storage = FlashStorage::new(flash, dma);
    let mut persistence = ConfigPersistence::new(flash_storage);

    let default_config = create_default_config();

    match persistence.load().await {
        Ok(config) => {
            info!("Loaded configuration from flash");
            config
        }
        Err(_) => {
            // Flash empty or invalid - use embedded defaults
            info!("No valid configuration in flash, using embedded defaults");
            default_config
        }
    }
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
    (&programs[..program_count], &profiles[..profile_count], &jars[..jar_count])
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
            error!("Failed to parse embedded config: {:?}", defmt::Debug2Format(&e));
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
        tower_pos: 0,
        lift_pos: 0,
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
