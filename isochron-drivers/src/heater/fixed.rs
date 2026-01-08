//! Fixed-point arithmetic for PID calculations
//!
//! Uses Q16.16 fixed-point format for coefficient storage.
//! This avoids hardware floating-point requirements on Cortex-M0.

use core::ops::{Add, Neg, Sub};

/// Q16.16 fixed-point number
///
/// Range: approximately -32768.0 to +32767.99998
/// Resolution: approximately 0.000015
///
/// Used for PID coefficients and intermediate calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Fixed32(pub i32);

impl Fixed32 {
    /// Zero value
    pub const ZERO: Self = Self(0);

    /// One (1.0)
    pub const ONE: Self = Self(1 << 16);

    /// Fractional bits (16)
    pub const FRAC_BITS: u32 = 16;

    /// Create from a whole integer
    ///
    /// # Example
    /// ```
    /// use isochron_drivers::heater::fixed::Fixed32;
    /// let two = Fixed32::from_int(2);
    /// assert_eq!(two.to_int(), 2);
    /// ```
    #[inline]
    pub const fn from_int(n: i16) -> Self {
        Self((n as i32) << Self::FRAC_BITS)
    }

    /// Create from a scaled integer (value × 100)
    ///
    /// This is useful for parsing config values like "1.50" stored as 150.
    ///
    /// # Example
    /// ```
    /// use isochron_drivers::heater::fixed::Fixed32;
    /// let one_point_five = Fixed32::from_scaled_100(150);
    /// assert_eq!(one_point_five.to_int(), 1);
    /// ```
    #[inline]
    pub const fn from_scaled_100(n: i32) -> Self {
        // (n << 16) / 100, but we need to be careful about overflow
        // For reasonable PID values (< 1000), this is safe
        Self((n << Self::FRAC_BITS) / 100)
    }

    /// Create from a scaled integer (value × 1000)
    ///
    /// Higher precision for small coefficients.
    #[inline]
    pub const fn from_scaled_1000(n: i32) -> Self {
        Self((n << Self::FRAC_BITS) / 1000)
    }

    /// Convert to whole integer (truncates fractional part)
    #[inline]
    pub const fn to_int(self) -> i16 {
        (self.0 >> Self::FRAC_BITS) as i16
    }

    /// Convert to scaled integer (value × 100)
    #[inline]
    pub const fn to_scaled_100(self) -> i32 {
        (self.0 * 100) >> Self::FRAC_BITS
    }

    /// Multiply two fixed-point numbers
    ///
    /// Uses i64 intermediate to avoid overflow.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: Self) -> Self {
        let result = ((self.0 as i64) * (other.0 as i64)) >> Self::FRAC_BITS;
        Self(result as i32)
    }

    /// Divide by another fixed-point number
    ///
    /// Returns ZERO if divisor is zero.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::ZERO;
        }
        let result = ((self.0 as i64) << Self::FRAC_BITS) / (other.0 as i64);
        Self(result as i32)
    }

    /// Divide by an integer
    ///
    /// Returns ZERO if divisor is zero.
    #[inline]
    pub fn div_int(self, divisor: i32) -> Self {
        if divisor == 0 {
            return Self::ZERO;
        }
        Self(self.0 / divisor)
    }

    /// Multiply by an integer
    #[inline]
    pub fn mul_int(self, n: i32) -> Self {
        Self(self.0.saturating_mul(n))
    }

    /// Saturating addition (clamps on overflow)
    #[inline]
    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction (clamps on underflow)
    #[inline]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Clamp value to a range
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }

    /// Absolute value
    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Check if value is negative
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Check if value is zero
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Get the raw i32 representation
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Create from raw i32 representation
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
}

impl Add for Fixed32 {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }
}

impl Sub for Fixed32 {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }
}

impl Neg for Fixed32 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl From<i16> for Fixed32 {
    fn from(n: i16) -> Self {
        Self::from_int(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_int() {
        assert_eq!(Fixed32::from_int(0).to_int(), 0);
        assert_eq!(Fixed32::from_int(1).to_int(), 1);
        assert_eq!(Fixed32::from_int(-1).to_int(), -1);
        assert_eq!(Fixed32::from_int(100).to_int(), 100);
        assert_eq!(Fixed32::from_int(-100).to_int(), -100);
    }

    #[test]
    fn test_from_scaled_100() {
        assert_eq!(Fixed32::from_scaled_100(100).to_int(), 1);
        assert_eq!(Fixed32::from_scaled_100(150).to_int(), 1); // 1.5 truncates to 1
        assert_eq!(Fixed32::from_scaled_100(250).to_int(), 2); // 2.5 truncates to 2
                                                               // Note: right-shift on signed integers is arithmetic (floor division),
                                                               // so -1.5 truncates to -2 (toward negative infinity), not -1
        assert_eq!(Fixed32::from_scaled_100(-150).to_int(), -2);
    }

    #[test]
    fn test_to_scaled_100() {
        assert_eq!(Fixed32::from_scaled_100(150).to_scaled_100(), 150);
        assert_eq!(Fixed32::from_scaled_100(75).to_scaled_100(), 75);
        assert_eq!(Fixed32::from_int(2).to_scaled_100(), 200);
    }

    #[test]
    fn test_multiply() {
        let two = Fixed32::from_int(2);
        let three = Fixed32::from_int(3);
        assert_eq!(two.mul(three).to_int(), 6);

        let half = Fixed32::from_scaled_100(50);
        assert_eq!(two.mul(half).to_int(), 1);

        let one_point_five = Fixed32::from_scaled_100(150);
        assert_eq!(two.mul(one_point_five).to_int(), 3);
    }

    #[test]
    fn test_divide() {
        let six = Fixed32::from_int(6);
        let two = Fixed32::from_int(2);
        assert_eq!(six.div(two).to_int(), 3);

        let ten = Fixed32::from_int(10);
        assert_eq!(ten.div_int(2).to_int(), 5);
    }

    #[test]
    fn test_saturating_add() {
        let a = Fixed32::from_int(100);
        let b = Fixed32::from_int(50);
        assert_eq!(a.saturating_add(b).to_int(), 150);

        // Test near max
        let big = Fixed32::from_int(32000);
        let also_big = Fixed32::from_int(1000);
        let result = big.saturating_add(also_big);
        assert!(result.to_int() > 0); // Should saturate, not wrap negative
    }

    #[test]
    fn test_clamp() {
        let value = Fixed32::from_int(50);
        let min = Fixed32::from_int(0);
        let max = Fixed32::from_int(100);
        assert_eq!(value.clamp(min, max).to_int(), 50);

        let too_low = Fixed32::from_int(-10);
        assert_eq!(too_low.clamp(min, max).to_int(), 0);

        let too_high = Fixed32::from_int(200);
        assert_eq!(too_high.clamp(min, max).to_int(), 100);
    }

    #[test]
    fn test_ops() {
        let a = Fixed32::from_int(5);
        let b = Fixed32::from_int(3);

        assert_eq!((a + b).to_int(), 8);
        assert_eq!((a - b).to_int(), 2);
        assert_eq!((-a).to_int(), -5);
    }
}
