//! Embassy async tasks
//!
//! Each task runs independently and communicates via channels/signals.

pub mod ac_motor;
pub mod calibration;
pub mod controller;
pub mod dc_motor;
pub mod display_rx;
pub mod display_tx;
pub mod heater;
pub mod stall_monitor;
pub mod stepper;
pub mod tick;
pub mod tmc;
pub mod x_stepper;
pub mod z_stepper;

pub use ac_motor::{ac_motor_task, AcMotorFwConfig};
pub use calibration::calibration_task;
pub use controller::controller_task;
pub use dc_motor::{dc_motor_task, DcMotorFwConfig};
pub use display_rx::display_rx_task;
pub use display_tx::display_tx_task;
pub use heater::{heater_task, HeaterConfig};
pub use stall_monitor::{stall_monitor_task, StallMonitorConfig};
pub use stepper::stepper_task;
pub use tick::tick_task;
pub use tmc::tmc_init_task;
pub use x_stepper::x_stepper_task;
pub use z_stepper::z_stepper_task;
