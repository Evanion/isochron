//! Embassy async tasks
//!
//! Each task runs independently and communicates via channels/signals.

pub mod ac_motor;
pub mod controller;
pub mod dc_motor;
pub mod display_rx;
pub mod display_tx;
pub mod heater;
pub mod stall_monitor;
pub mod stepper;
pub mod tick;
pub mod tmc;

pub use ac_motor::{ac_motor_task, AcMotorFwConfig};
pub use controller::controller_task;
pub use dc_motor::{dc_motor_task, DcMotorFwConfig};
pub use display_rx::display_rx_task;
pub use display_tx::display_tx_task;
pub use heater::{heater_task, HeaterConfig};
pub use stall_monitor::{stall_monitor_task, StallMonitorConfig};
pub use stepper::stepper_task;
pub use tick::tick_task;
pub use tmc::tmc_init_task;
