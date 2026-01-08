//! Isochron Display Firmware
//!
//! Firmware for the V0 Mini OLED display module (STM32F042K6).
//! Communicates with the main controller via UART protocol.

#![no_std]
#![no_main]

mod encoder;
mod font;
mod protocol;
mod sh1106;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Pull};
use embassy_stm32::i2c::{self, I2c};
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::{I2C1, USART2};
use embassy_stm32::usart::{self, Uart};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker, Timer};
use {defmt_rtt as _, panic_probe as _};

use crate::encoder::Encoder;
use crate::sh1106::Sh1106;
use isochron_protocol::{ControllerCommand, DisplayCommand, FrameParser, InputEvent};

use embassy_stm32::exti;

bind_interrupts!(struct Irqs {
    USART2 => usart::InterruptHandler<USART2>;
    I2C1 => i2c::EventInterruptHandler<I2C1>, i2c::ErrorInterruptHandler<I2C1>;
    EXTI0_1 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0_1>;
});

/// Display state for rendering
pub struct DisplayState {
    pub lines: [heapless::String<21>; 8],
    pub invert: Option<(u8, u8, u8)>, // row, start, end
    pub dirty: bool,
}

impl DisplayState {
    pub const fn new() -> Self {
        Self {
            lines: [
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
            ],
            invert: None,
            dirty: true,
        }
    }

    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.invert = None;
        self.dirty = true;
    }

    pub fn set_text(&mut self, row: u8, col: u8, text: &str) {
        if row < 8 {
            let line = &mut self.lines[row as usize];
            // Pad with spaces if needed
            while line.len() < col as usize {
                let _ = line.push(' ');
            }
            // Truncate if col is beyond current length
            if col as usize <= line.len() {
                line.truncate(col as usize);
            }
            // Append new text
            for ch in text.chars() {
                if line.len() >= 21 {
                    break;
                }
                let _ = line.push(ch);
            }
            self.dirty = true;
        }
    }
}

/// Shared display state
static DISPLAY_STATE: Mutex<CriticalSectionRawMutex, DisplayState> =
    Mutex::new(DisplayState::new());

/// Signal to trigger display refresh
static DISPLAY_REFRESH: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Signal for input events to send to controller
static INPUT_EVENT: Signal<CriticalSectionRawMutex, InputEvent> = Signal::new();

/// Heartbeat interval
const HEARTBEAT_MS: u64 = 1000;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Isochron Display Firmware starting...");

    let p = embassy_stm32::init(Default::default());

    // Setup I2C for OLED (PB6=SCL, PB7=SDA)
    let mut i2c_config = i2c::Config::default();
    i2c_config.timeout = Duration::from_millis(100);

    let i2c = I2c::new(
        p.I2C1, p.PB6, p.PB7, Irqs, p.DMA1_CH2, p.DMA1_CH3, i2c_config,
    );

    // Initialize OLED display
    let mut display = Sh1106::new(i2c);
    if let Err(e) = display.init().await {
        error!("Failed to initialize display: {:?}", e);
    } else {
        info!("OLED initialized");
        display.clear().await.ok();
        display.draw_text(0, 0, "Isochron").await.ok();
        display.draw_text(1, 0, "Display v0.1").await.ok();
        display.flush().await.ok();
    }

    // Setup UART for controller communication (PA2=TX, PA3=RX on F042K6)
    let mut uart_config = usart::Config::default();
    uart_config.baudrate = 115200;

    let uart = Uart::new(
        p.USART2,
        p.PA3, // RX
        p.PA2, // TX
        Irqs,
        p.DMA1_CH4,
        p.DMA1_CH5,
        uart_config,
    )
    .unwrap();

    let (tx, rx) = uart.split();

    // Setup encoder (PA4=A, PA5=B, PA1=Button)
    let enc_a = Input::new(p.PA4, Pull::Up);
    let enc_b = Input::new(p.PA5, Pull::Up);
    let enc_btn = ExtiInput::new(p.PA1, p.EXTI1, Pull::Up, Irqs);

    // Spawn tasks
    spawner.spawn(uart_rx_task(rx)).unwrap();
    spawner.spawn(uart_tx_task(tx)).unwrap();
    spawner.spawn(encoder_task(enc_a, enc_b)).unwrap();
    spawner.spawn(button_task(enc_btn)).unwrap();
    spawner.spawn(display_task(display)).unwrap();

    info!("All tasks spawned");
}

/// UART receive task - receives commands from controller
#[embassy_executor::task]
async fn uart_rx_task(mut rx: usart::UartRx<'static, Async>) {
    info!("UART RX task started");

    let mut parser = FrameParser::new();
    let mut buf = [0u8; 1];

    loop {
        match rx.read(&mut buf).await {
            Ok(()) => {
                if let Ok(Some(frame)) = parser.feed(buf[0]) {
                    match ControllerCommand::from_frame(&frame) {
                        Ok(cmd) => {
                            handle_controller_command(cmd).await;
                        }
                        Err(e) => {
                            warn!("Failed to parse command: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("UART read error: {:?}", e);
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}

/// Handle a command from the controller
async fn handle_controller_command(cmd: ControllerCommand) {
    match cmd {
        ControllerCommand::Ping => {
            trace!("PING received");
            // Pong is sent in TX task via heartbeat
        }
        ControllerCommand::ClearScreen => {
            trace!("Clear screen");
            {
                let mut state = DISPLAY_STATE.lock().await;
                state.clear();
            }
            DISPLAY_REFRESH.signal(());
        }
        ControllerCommand::Text { row, col, text } => {
            trace!("Text at ({}, {}): {}", row, col, text.as_str());
            {
                let mut state = DISPLAY_STATE.lock().await;
                state.set_text(row, col, text.as_str());
            }
            DISPLAY_REFRESH.signal(());
        }
        ControllerCommand::Invert {
            row,
            start_col,
            end_col,
        } => {
            trace!("Invert row {} cols {}-{}", row, start_col, end_col);
            {
                let mut state = DISPLAY_STATE.lock().await;
                state.invert = Some((row, start_col, end_col));
                state.dirty = true;
            }
            DISPLAY_REFRESH.signal(());
        }
        ControllerCommand::Reset => {
            info!("Reset requested");
            {
                let mut state = DISPLAY_STATE.lock().await;
                state.clear();
            }
            DISPLAY_REFRESH.signal(());
        }
    }
}

/// UART transmit task - sends events and heartbeats to controller
#[embassy_executor::task]
async fn uart_tx_task(mut tx: usart::UartTx<'static, Async>) {
    info!("UART TX task started");

    let mut heartbeat = Ticker::every(Duration::from_millis(HEARTBEAT_MS));
    let mut buf = [0u8; 64];

    loop {
        // Check for input events (non-blocking)
        if let Some(event) = INPUT_EVENT.try_take() {
            if let Ok(frame) = DisplayCommand::Input(event).to_frame() {
                if let Ok(len) = frame.encode(&mut buf) {
                    tx.write(&buf[..len]).await.ok();
                    trace!("Sent input event");
                }
            }
        }

        // Send periodic heartbeat (PING)
        heartbeat.next().await;
        if let Ok(frame) = DisplayCommand::Ping.to_frame() {
            if let Ok(len) = frame.encode(&mut buf) {
                tx.write(&buf[..len]).await.ok();
                trace!("Sent heartbeat");
            }
        }
    }
}

/// Encoder rotation task
#[embassy_executor::task]
async fn encoder_task(a: Input<'static>, b: Input<'static>) {
    info!("Encoder task started");

    let mut encoder = Encoder::new(a, b);

    loop {
        if let Some(event) = encoder.poll().await {
            INPUT_EVENT.signal(event);
        }
    }
}

/// Button press task
#[embassy_executor::task]
async fn button_task(mut btn: ExtiInput<'static>) {
    info!("Button task started");

    loop {
        btn.wait_for_falling_edge().await;
        let press_start = embassy_time::Instant::now();

        // Debounce
        Timer::after(Duration::from_millis(20)).await;

        if btn.is_low() {
            // Wait for release or long press timeout
            let long_press =
                embassy_time::with_timeout(Duration::from_millis(500), btn.wait_for_rising_edge())
                    .await;

            match long_press {
                Ok(()) => {
                    // Short press
                    let duration = press_start.elapsed();
                    if duration.as_millis() > 50 {
                        INPUT_EVENT.signal(InputEvent::EncoderClick);
                        debug!("Button: Click");
                    }
                }
                Err(_) => {
                    // Long press timeout
                    INPUT_EVENT.signal(InputEvent::EncoderLongPress);
                    debug!("Button: LongPress");
                    // Wait for actual release
                    btn.wait_for_rising_edge().await;
                }
            }

            // Debounce after release
            Timer::after(Duration::from_millis(50)).await;
        }
    }
}

/// Display update task
#[embassy_executor::task]
async fn display_task(mut display: Sh1106<I2c<'static, Async, embassy_stm32::i2c::Master>>) {
    info!("Display task started");

    loop {
        // Wait for refresh signal
        DISPLAY_REFRESH.wait().await;

        // Lock state and render
        let state = DISPLAY_STATE.lock().await;

        if state.dirty {
            display.clear().await.ok();

            for (row, line) in state.lines.iter().enumerate() {
                if !line.is_empty() {
                    display.draw_text(row as u8, 0, line.as_str()).await.ok();
                }
            }

            // Handle invert region
            if let Some((row, start, end)) = state.invert {
                display.invert_region(row, start, end).await.ok();
            }

            display.flush().await.ok();
            trace!("Display updated");
        }

        // Note: We don't mark clean here because we don't have mutable access
        // The dirty flag will be reset on next clear or handled differently
    }
}
