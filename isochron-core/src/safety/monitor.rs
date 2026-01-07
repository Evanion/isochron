//! Safety monitor implementation
//!
//! Monitors temperature, motor stall, and communication link health.

use crate::state::ErrorKind;

/// Safety thresholds
pub const MAX_TEMPERATURE_C: i16 = 55;
pub const HEARTBEAT_TIMEOUT_MS: u32 = 3000;
pub const MAX_MISSED_HEARTBEATS: u8 = 3;

/// Safety condition status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SafetyStatus {
    /// All conditions normal
    Ok,
    /// Safety condition violated
    Fault(ErrorKind),
}

/// Safety monitor for fault detection
///
/// This struct tracks safety-related state and determines
/// when to trigger error conditions.
#[derive(Debug, Clone)]
pub struct SafetyMonitor {
    /// Current temperature reading (×10 for 0.1°C resolution)
    last_temp_x10: Option<i16>,
    /// Temperature sensor valid
    temp_sensor_valid: bool,
    /// Motor stall detected
    motor_stalled: bool,
    /// Missed heartbeat count
    missed_heartbeats: u8,
    /// Time since last heartbeat (ms)
    time_since_heartbeat_ms: u32,
}

impl Default for SafetyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyMonitor {
    /// Create a new safety monitor
    pub fn new() -> Self {
        Self {
            last_temp_x10: None,
            temp_sensor_valid: true,
            motor_stalled: false,
            missed_heartbeats: 0,
            time_since_heartbeat_ms: 0,
        }
    }

    /// Update temperature reading
    ///
    /// # Arguments
    /// - `temp_x10`: Temperature in 0.1°C units, or None if sensor fault
    pub fn update_temperature(&mut self, temp_x10: Option<i16>) {
        self.last_temp_x10 = temp_x10;
        self.temp_sensor_valid = temp_x10.is_some();
    }

    /// Update motor stall status
    pub fn update_motor_stall(&mut self, stalled: bool) {
        self.motor_stalled = stalled;
    }

    /// Record a heartbeat received
    pub fn heartbeat_received(&mut self) {
        self.missed_heartbeats = 0;
        self.time_since_heartbeat_ms = 0;
    }

    /// Update time tracking
    ///
    /// # Arguments
    /// - `delta_ms`: Time elapsed since last update
    pub fn update_time(&mut self, delta_ms: u32) {
        self.time_since_heartbeat_ms = self.time_since_heartbeat_ms.saturating_add(delta_ms);

        if self.time_since_heartbeat_ms >= HEARTBEAT_TIMEOUT_MS {
            self.missed_heartbeats = self.missed_heartbeats.saturating_add(1);
            self.time_since_heartbeat_ms = 0;
        }
    }

    /// Check all safety conditions
    ///
    /// Returns the first fault detected, or Ok if all conditions are normal.
    pub fn check(&self) -> SafetyStatus {
        // Check temperature sensor fault
        if !self.temp_sensor_valid {
            return SafetyStatus::Fault(ErrorKind::ThermistorFault);
        }

        // Check over-temperature
        if let Some(temp_x10) = self.last_temp_x10 {
            if temp_x10 > MAX_TEMPERATURE_C * 10 {
                return SafetyStatus::Fault(ErrorKind::OverTemperature);
            }
        }

        // Check motor stall
        if self.motor_stalled {
            return SafetyStatus::Fault(ErrorKind::MotorStall);
        }

        // Check link health
        if self.missed_heartbeats >= MAX_MISSED_HEARTBEATS {
            return SafetyStatus::Fault(ErrorKind::LinkLost);
        }

        SafetyStatus::Ok
    }

    /// Get current temperature in whole degrees Celsius
    pub fn get_temperature(&self) -> Option<i16> {
        self.last_temp_x10.map(|t| t / 10)
    }

    /// Check if link is healthy
    pub fn is_link_healthy(&self) -> bool {
        self.missed_heartbeats < MAX_MISSED_HEARTBEATS
    }

    /// Get number of missed heartbeats
    pub fn get_missed_heartbeats(&self) -> u8 {
        self.missed_heartbeats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_operation() {
        let mut monitor = SafetyMonitor::new();
        monitor.update_temperature(Some(450)); // 45.0°C
        assert_eq!(monitor.check(), SafetyStatus::Ok);
    }

    #[test]
    fn test_over_temperature() {
        let mut monitor = SafetyMonitor::new();
        monitor.update_temperature(Some(560)); // 56.0°C > 55°C
        assert_eq!(
            monitor.check(),
            SafetyStatus::Fault(ErrorKind::OverTemperature)
        );
    }

    #[test]
    fn test_sensor_fault() {
        let mut monitor = SafetyMonitor::new();
        monitor.update_temperature(None);
        assert_eq!(
            monitor.check(),
            SafetyStatus::Fault(ErrorKind::ThermistorFault)
        );
    }

    #[test]
    fn test_motor_stall() {
        let mut monitor = SafetyMonitor::new();
        monitor.update_temperature(Some(400));
        monitor.update_motor_stall(true);
        assert_eq!(
            monitor.check(),
            SafetyStatus::Fault(ErrorKind::MotorStall)
        );
    }

    #[test]
    fn test_link_lost() {
        let mut monitor = SafetyMonitor::new();
        monitor.update_temperature(Some(400));

        // Miss 3 heartbeats
        for _ in 0..3 {
            monitor.update_time(HEARTBEAT_TIMEOUT_MS);
        }

        assert_eq!(
            monitor.check(),
            SafetyStatus::Fault(ErrorKind::LinkLost)
        );
    }

    #[test]
    fn test_heartbeat_resets_counter() {
        let mut monitor = SafetyMonitor::new();

        // Miss 2 heartbeats
        monitor.update_time(HEARTBEAT_TIMEOUT_MS);
        monitor.update_time(HEARTBEAT_TIMEOUT_MS);
        assert_eq!(monitor.get_missed_heartbeats(), 2);

        // Receive heartbeat
        monitor.heartbeat_received();
        assert_eq!(monitor.get_missed_heartbeats(), 0);
        assert!(monitor.is_link_healthy());
    }
}
