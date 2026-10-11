//! Fixed-point scalar used for every public measurement.
//!
//! `Fx` is a 16.16 signed fixed-point number stored in an `i32`. Arithmetic
//! is integer-only and saturates instead of panicking, so layout output is
//! bit-identical on every platform (no float rounding differences between
//! x86, ARM and WASM).

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

const FRAC_BITS: u32 = 16;
const ONE_RAW: i64 = 1 << FRAC_BITS;

/// A 16.16 fixed-point number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Fx(i32);

impl Fx {
    /// Zero.
    pub const ZERO: Fx = Fx(0);
    /// One.
    pub const ONE: Fx = Fx(1 << FRAC_BITS);
    /// The largest representable value.
    pub const MAX: Fx = Fx(i32::MAX);
    /// The smallest representable value.
    pub const MIN: Fx = Fx(i32::MIN);

    /// Builds a value from its raw 16.16 representation.
    pub const fn from_raw(raw: i32) -> Fx {
        Fx(raw)
    }

    /// Returns the raw 16.16 representation.
    pub const fn to_raw(self) -> i32 {
        self.0
    }

    /// Builds a value from an integer, saturating on overflow.
    pub fn from_int(value: i32) -> Fx {
        Fx(saturate((value as i64) << FRAC_BITS))
    }

    /// Converts from `f32`, rounding half away from zero and saturating.
    /// NaN becomes zero.
    ///
    /// Only use this at API boundaries; prefer `Fx` arithmetic internally.
    pub fn from_f32(value: f32) -> Fx {
        if value.is_nan() {
            return Fx::ZERO;
        }
        let scaled = value * ONE_RAW as f32;
        // `as` casts from float saturate and are deterministic.
        let rounded = if scaled >= 0.0 {
            (scaled + 0.5) as i64
        } else {
            (scaled - 0.5) as i64
        };
        Fx(saturate(rounded))
    }

    /// Converts to `f32`. Lossy for very large magnitudes.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / ONE_RAW as f32
    }

    /// Rounds to the nearest integer, half away from zero.
    pub fn round(self) -> i32 {
        let half = ONE_RAW / 2;
        let raw = self.0 as i64;
        let rounded = if raw >= 0 {
            (raw + half) >> FRAC_BITS
        } else {
            -((-raw + half) >> FRAC_BITS)
        };
        saturate(rounded)
    }

    /// Returns the larger of two values.
    pub fn max(self, other: Fx) -> Fx {
        if self >= other {
            self
        } else {
            other
        }
    }

    /// Returns the smaller of two values.
    pub fn min(self, other: Fx) -> Fx {
        if self <= other {
            self
        } else {
            other
        }
    }

    /// Returns `true` if the value is strictly negative.
    pub fn is_negative(self) -> bool {
        self.0 < 0
    }
}

/// Clamps an `i64` into `i32` range.
fn saturate(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// Computes `value * num / den`, rounding half away from zero.
///
/// Used to scale font units to `Fx` without floating point. Returns zero
/// when `den` is zero.
pub(crate) fn scale_round(value: i64, num: i64, den: i64) -> i64 {
    if den == 0 {
        return 0;
    }
    let product = value * num;
    let half = den.abs() / 2;
    let sign = if (product < 0) != (den < 0) { -1 } else { 1 };
    sign * ((product.abs() + half) / den.abs())
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Fx {
    type Output = Fx;
    fn sub(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_sub(rhs.0))
    }
}

impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(self.0.saturating_neg())
    }
}

impl Mul for Fx {
    type Output = Fx;
    fn mul(self, rhs: Fx) -> Fx {
        Fx(saturate(scale_round(self.0 as i64, rhs.0 as i64, ONE_RAW)))
    }
}

impl Div for Fx {
    type Output = Fx;
    /// Division by zero yields zero rather than panicking.
    fn div(self, rhs: Fx) -> Fx {
        if rhs.0 == 0 {
            return Fx::ZERO;
        }
        Fx(saturate(scale_round(self.0 as i64, ONE_RAW, rhs.0 as i64)))
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, rhs: Fx) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fx {
    fn sub_assign(&mut self, rhs: Fx) {
        *self = *self - rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_round_trip() {
        assert_eq!(Fx::from_int(12).round(), 12);
        assert_eq!(Fx::from_int(-7).round(), -7);
        assert_eq!(Fx::ONE.to_raw(), 65536);
    }

    #[test]
    fn rounding_is_half_away_from_zero() {
        assert_eq!(Fx::from_f32(2.5).round(), 3);
        assert_eq!(Fx::from_f32(-2.5).round(), -3);
        assert_eq!(Fx::from_f32(2.49).round(), 2);
    }

    #[test]
    fn multiplication_and_division() {
        let three = Fx::from_int(3);
        let half = Fx::from_f32(0.5);
        assert_eq!((three * half).to_f32(), 1.5);
        assert_eq!((three / Fx::from_int(2)).to_f32(), 1.5);
        assert_eq!(three / Fx::ZERO, Fx::ZERO);
    }

    #[test]
    fn arithmetic_saturates() {
        assert_eq!(Fx::MAX + Fx::ONE, Fx::MAX);
        assert_eq!(Fx::MIN - Fx::ONE, Fx::MIN);
        assert_eq!(Fx::from_f32(f32::NAN), Fx::ZERO);
    }

    #[test]
    fn scale_round_is_exact() {
        assert_eq!(scale_round(1000, 3, 4), 750);
        assert_eq!(scale_round(-1000, 1, 3), -333);
    }
}
