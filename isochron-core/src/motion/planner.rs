//! Motion planner for acceleration/deceleration profiles
//!
//! Provides smooth speed transitions to minimize fluid shock and vortex formation.

/// Default acceleration rate in RPM per second
pub const DEFAULT_ACCEL_RPM_PER_S: u16 = 50;

/// Maximum acceleration rate in RPM per second
pub const MAX_ACCEL_RPM_PER_S: u16 = 100;

/// Current motion state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MotionState {
    /// Motor is stopped
    Stopped,
    /// Motor is accelerating toward target
    Accelerating,
    /// Motor is at target speed
    AtSpeed,
    /// Motor is decelerating toward stop
    Decelerating,
}

/// Motion planner for smooth speed transitions
///
/// This planner calculates the current RPM based on target RPM and
/// acceleration parameters. It handles ramping up and down smoothly.
#[derive(Debug, Clone)]
pub struct MotionPlanner {
    /// Current actual RPM (fixed point: RPM * 10 for 0.1 RPM resolution)
    current_rpm_x10: u32,
    /// Target RPM
    target_rpm: u16,
    /// Acceleration rate in RPM/s
    accel_rpm_per_s: u16,
    /// Current motion state
    state: MotionState,
}

impl Default for MotionPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl MotionPlanner {
    /// Create a new motion planner with default acceleration
    pub fn new() -> Self {
        Self {
            current_rpm_x10: 0,
            target_rpm: 0,
            accel_rpm_per_s: DEFAULT_ACCEL_RPM_PER_S,
            state: MotionState::Stopped,
        }
    }

    /// Create a motion planner with custom acceleration
    pub fn with_acceleration(accel_rpm_per_s: u16) -> Self {
        Self {
            current_rpm_x10: 0,
            target_rpm: 0,
            accel_rpm_per_s: accel_rpm_per_s.min(MAX_ACCEL_RPM_PER_S),
            state: MotionState::Stopped,
        }
    }

    /// Set the target RPM
    pub fn set_target(&mut self, rpm: u16) {
        self.target_rpm = rpm;
        self.update_state();
    }

    /// Get the target RPM
    pub fn get_target(&self) -> u16 {
        self.target_rpm
    }

    /// Get the current actual RPM (may be ramping)
    pub fn get_current(&self) -> u16 {
        (self.current_rpm_x10 / 10) as u16
    }

    /// Get the current motion state
    pub fn get_state(&self) -> MotionState {
        self.state
    }

    /// Check if at target speed
    pub fn is_at_target(&self) -> bool {
        self.state == MotionState::AtSpeed || self.state == MotionState::Stopped
    }

    /// Check if stopped
    pub fn is_stopped(&self) -> bool {
        self.state == MotionState::Stopped && self.current_rpm_x10 == 0
    }

    /// Update the planner with elapsed time
    ///
    /// Call this periodically (e.g., every 10ms) to update the current RPM.
    ///
    /// # Arguments
    /// - `delta_ms`: Time elapsed since last update in milliseconds
    ///
    /// # Returns
    /// The current RPM after the update
    pub fn update(&mut self, delta_ms: u32) -> u16 {
        let target_x10 = (self.target_rpm as u32) * 10;

        if self.current_rpm_x10 == target_x10 {
            self.update_state();
            return self.get_current();
        }

        // Calculate change in RPM*10 for this time step
        // accel_rpm_per_s * 10 * delta_ms / 1000 = delta_rpm_x10
        let delta_x10 = (self.accel_rpm_per_s as u32) * 10 * delta_ms / 1000;

        if self.current_rpm_x10 < target_x10 {
            // Accelerating
            self.current_rpm_x10 = (self.current_rpm_x10 + delta_x10).min(target_x10);
        } else {
            // Decelerating
            if delta_x10 >= self.current_rpm_x10 {
                self.current_rpm_x10 = 0;
            } else {
                self.current_rpm_x10 = self.current_rpm_x10.saturating_sub(delta_x10);
            }
            self.current_rpm_x10 = self.current_rpm_x10.max(target_x10);
        }

        self.update_state();
        self.get_current()
    }

    /// Immediately stop (emergency stop)
    pub fn emergency_stop(&mut self) {
        self.target_rpm = 0;
        self.current_rpm_x10 = 0;
        self.state = MotionState::Stopped;
    }

    /// Update the motion state based on current/target RPM
    fn update_state(&mut self) {
        let target_x10 = (self.target_rpm as u32) * 10;

        if self.current_rpm_x10 == 0 && target_x10 == 0 {
            self.state = MotionState::Stopped;
        } else if self.current_rpm_x10 < target_x10 {
            self.state = MotionState::Accelerating;
        } else if self.current_rpm_x10 > target_x10 {
            self.state = MotionState::Decelerating;
        } else {
            self.state = MotionState::AtSpeed;
        }
    }

    /// Calculate time to reach target from current speed
    ///
    /// Returns time in milliseconds
    pub fn time_to_target(&self) -> u32 {
        let target_x10 = (self.target_rpm as u32) * 10;
        let diff = self.current_rpm_x10.abs_diff(target_x10);

        // time_ms = diff / (accel * 10 / 1000) = diff * 1000 / (accel * 10)
        if self.accel_rpm_per_s == 0 {
            return u32::MAX;
        }
        diff * 100 / (self.accel_rpm_per_s as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let planner = MotionPlanner::new();
        assert_eq!(planner.get_current(), 0);
        assert_eq!(planner.get_state(), MotionState::Stopped);
        assert!(planner.is_stopped());
    }

    #[test]
    fn test_acceleration() {
        let mut planner = MotionPlanner::with_acceleration(100); // 100 RPM/s
        planner.set_target(100);

        assert_eq!(planner.get_state(), MotionState::Accelerating);

        // After 500ms, should be at 50 RPM
        planner.update(500);
        assert_eq!(planner.get_current(), 50);

        // After another 500ms, should be at 100 RPM
        planner.update(500);
        assert_eq!(planner.get_current(), 100);
        assert_eq!(planner.get_state(), MotionState::AtSpeed);
    }

    #[test]
    fn test_deceleration() {
        let mut planner = MotionPlanner::with_acceleration(100);
        planner.set_target(100);
        planner.update(1000); // Get to 100 RPM
        assert_eq!(planner.get_current(), 100);

        // Start decelerating
        planner.set_target(0);
        assert_eq!(planner.get_state(), MotionState::Decelerating);

        // After 500ms, should be at 50 RPM
        planner.update(500);
        assert_eq!(planner.get_current(), 50);

        // After another 500ms, should be stopped
        planner.update(500);
        assert_eq!(planner.get_current(), 0);
        assert!(planner.is_stopped());
    }

    #[test]
    fn test_emergency_stop() {
        let mut planner = MotionPlanner::new();
        planner.set_target(100);
        planner.update(500);
        assert!(planner.get_current() > 0);

        planner.emergency_stop();
        assert_eq!(planner.get_current(), 0);
        assert!(planner.is_stopped());
    }

    #[test]
    fn test_time_to_target() {
        let planner = MotionPlanner::with_acceleration(100);
        // 0 to 100 RPM at 100 RPM/s = 1000ms
        assert_eq!(planner.time_to_target(), 0); // Already at target (0)

        let mut planner = MotionPlanner::with_acceleration(100);
        planner.set_target(100);
        // Should take ~1000ms to reach target
        assert_eq!(planner.time_to_target(), 1000);
    }
}
