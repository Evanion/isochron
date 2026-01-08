//! Execution segments generated from profiles

use crate::traits::Direction;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A single execution segment
///
/// Segments are the atomic units of execution. Each segment has a
/// direction and duration. Direction changes happen between segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Segment {
    /// Rotation direction
    pub direction: Direction,
    /// Duration in seconds
    pub duration_s: u16,
    /// Target RPM for this segment
    pub rpm: u16,
}

/// Spin-off configuration for a profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SpinOffConfig {
    /// Height to lift basket above jar (mm)
    pub lift_mm: u16,
    /// Spin speed during spin-off
    pub rpm: u16,
    /// Spin-off duration (seconds)
    pub time_s: u16,
}

/// Direction mode for profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DirectionMode {
    /// Continuous clockwise rotation
    #[default]
    Clockwise,
    /// Continuous counter-clockwise rotation
    CounterClockwise,
    /// Alternating direction
    Alternate,
}

/// Minimum segment duration in seconds
pub const MIN_SEGMENT_DURATION_S: u16 = 10;

/// Generate segments from profile parameters
///
/// # Arguments
/// - `rpm`: Target rotation speed
/// - `total_time_s`: Total profile duration
/// - `direction`: Direction mode
/// - `iterations`: Number of alternations (only used for Alternate mode)
///
/// # Returns
/// A vector of segments, or None if validation fails
pub fn generate_segments(
    rpm: u16,
    total_time_s: u16,
    direction: DirectionMode,
    iterations: u8,
) -> Option<heapless::Vec<Segment, 16>> {
    use heapless::Vec;

    let mut segments = Vec::new();

    match direction {
        DirectionMode::Clockwise => {
            segments
                .push(Segment {
                    direction: Direction::Clockwise,
                    duration_s: total_time_s,
                    rpm,
                })
                .ok()?;
        }
        DirectionMode::CounterClockwise => {
            segments
                .push(Segment {
                    direction: Direction::CounterClockwise,
                    duration_s: total_time_s,
                    rpm,
                })
                .ok()?;
        }
        DirectionMode::Alternate => {
            if iterations == 0 {
                return None;
            }

            let num_segments = (iterations as u16) * 2;
            let segment_duration = total_time_s / num_segments;

            // Validate minimum segment duration
            if segment_duration < MIN_SEGMENT_DURATION_S {
                return None;
            }

            let mut current_dir = Direction::Clockwise;
            for _ in 0..num_segments {
                segments
                    .push(Segment {
                        direction: current_dir,
                        duration_s: segment_duration,
                        rpm,
                    })
                    .ok()?;
                current_dir = current_dir.opposite();
            }
        }
    }

    Some(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_direction() {
        let segments = generate_segments(120, 180, DirectionMode::Clockwise, 0).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].direction, Direction::Clockwise);
        assert_eq!(segments[0].duration_s, 180);
        assert_eq!(segments[0].rpm, 120);
    }

    #[test]
    fn test_alternate_direction() {
        let segments = generate_segments(120, 180, DirectionMode::Alternate, 3).unwrap();

        // 3 iterations * 2 segments = 6 segments
        assert_eq!(segments.len(), 6);

        // Each segment is 180 / 6 = 30 seconds
        assert_eq!(segments[0].duration_s, 30);

        // Directions alternate
        assert_eq!(segments[0].direction, Direction::Clockwise);
        assert_eq!(segments[1].direction, Direction::CounterClockwise);
        assert_eq!(segments[2].direction, Direction::Clockwise);
    }

    #[test]
    fn test_alternate_zero_iterations() {
        let result = generate_segments(120, 180, DirectionMode::Alternate, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_segment_too_short() {
        // 60 seconds / 8 segments = 7.5 seconds < MIN_SEGMENT_DURATION_S
        let result = generate_segments(120, 60, DirectionMode::Alternate, 4);
        assert!(result.is_none());
    }
}
