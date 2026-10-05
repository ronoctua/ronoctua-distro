//! The arithmetic of [`Number`]: exact while it can be, approximate after.
//!
//! Every operation that can fail says why with a [`Why`]; the caller adds
//! where. An approximation that is not a number (`NaN`) is [`Why::Domain`],
//! and one that overflowed is [`Why::TooLarge`] — never a result.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One as _, Signed as _, ToPrimitive as _, Zero as _};

use crate::{Number, Why};

/// The most bits an exact result may take before it is refused: a power of a
/// power (`9^9^9`) would otherwise fill the memory of the machine.
const MOST_BITS: u64 = 1 << 16;

/// The largest factorial computed exactly.
const MOST_FACTORIAL: u32 = 3000;

pub(crate) fn exact(n: i64, d: i64) -> Number {
    Number::Exact(BigRational::new(BigInt::from(n), BigInt::from(d)))
}

pub(crate) fn ratio(pair: (i128, i128)) -> BigRational {
    BigRational::new(BigInt::from(pair.0), BigInt::from(pair.1))
}

/// An approximation, refused when it is not a finite number.
pub(crate) fn approx(f: f64) -> Result<Number, Why> {
    if f.is_nan() {
        Err(Why::Domain)
    } else if f.is_infinite() {
        Err(Why::TooLarge)
    } else {
        Ok(Number::Approx(f))
    }
}

fn bits(r: &BigRational) -> u64 {
    r.numer().bits() + r.denom().bits()
}

impl Number {
    pub(crate) fn zero() -> Self {
        Self::Exact(BigRational::zero())
    }

    pub(crate) fn one() -> Self {
        Self::Exact(BigRational::one())
    }

    pub(crate) fn is_zero(&self) -> bool {
        match self {
            Self::Exact(r) => r.is_zero(),
            Self::Approx(f) => *f == 0.0,
        }
    }

    pub(crate) fn is_negative(&self) -> bool {
        match self {
            Self::Exact(r) => r.is_negative(),
            Self::Approx(f) => *f < 0.0,
        }
    }

    /// The integer it is, when it is an exact one.
    pub(crate) fn integer(&self) -> Option<BigInt> {
        match self {
            Self::Exact(r) if r.is_integer() => Some(r.to_integer()),
            _ => None,
        }
    }

    pub(crate) fn neg(&self) -> Self {
        match self {
            Self::Exact(r) => Self::Exact(-r),
            Self::Approx(f) => Self::Approx(-f),
        }
    }

    pub(crate) fn add(&self, other: &Self) -> Result<Self, Why> {
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => Ok(Self::Exact(a + b)),
            _ => approx(self.to_f64() + other.to_f64()),
        }
    }

    pub(crate) fn sub(&self, other: &Self) -> Result<Self, Why> {
        self.add(&other.neg())
    }

    pub(crate) fn mul(&self, other: &Self) -> Result<Self, Why> {
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => {
                if bits(a) + bits(b) > MOST_BITS {
                    return Err(Why::TooLarge);
                }
                Ok(Self::Exact(a * b))
            }
            _ => approx(self.to_f64() * other.to_f64()),
        }
    }

    pub(crate) fn div(&self, other: &Self) -> Result<Self, Why> {
        if other.is_zero() {
            return Err(Why::DivideByZero);
        }
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => {
                if bits(a) + bits(b) > MOST_BITS {
                    return Err(Why::TooLarge);
                }
                Ok(Self::Exact(a / b))
            }
            _ => approx(self.to_f64() / other.to_f64()),
        }
    }

    /// The reciprocal.
    pub(crate) fn recip(&self) -> Result<Self, Why> {
        Self::one().div(self)
    }

    /// `self` to the power `exponent`: exact for an exact base and an integer
    /// exponent, approximate otherwise.
    pub(crate) fn pow(&self, exponent: &Self) -> Result<Self, Why> {
        if let (Self::Exact(base), Some(n)) = (self, exponent.integer()) {
            let n = n.to_i32().ok_or(Why::TooLarge)?;
            // `0^0` is 1, as the empty product is.
            if n == 0 {
                return Ok(Self::one());
            }
            if base.is_zero() && n < 0 {
                return Err(Why::DivideByZero);
            }
            if base.is_zero() || base.is_one() {
                return Ok(self.clone());
            }
            let grown = bits(base).saturating_mul(u64::from(n.unsigned_abs()));
            if grown > MOST_BITS {
                return Err(Why::TooLarge);
            }
            return Ok(Self::Exact(num_traits::Pow::pow(base, n)));
        }
        let (b, e) = (self.to_f64(), exponent.to_f64());
        if b < 0.0 && e.fract() != 0.0 {
            return Err(Why::Domain);
        }
        approx(b.powf(e))
    }

    /// The factorial of a non-negative integer.
    pub(crate) fn factorial(&self) -> Result<Self, Why> {
        let n = self.integer().ok_or(Why::Domain)?;
        if n.is_negative() {
            return Err(Why::Domain);
        }
        let n = n
            .to_u32()
            .filter(|n| *n <= MOST_FACTORIAL)
            .ok_or(Why::TooLarge)?;
        let mut out = BigInt::one();
        for k in 2..=n {
            out *= k;
        }
        Ok(Self::Exact(BigRational::from_integer(out)))
    }

    /// The square root: exact for a rational square, approximate otherwise.
    pub(crate) fn sqrt(&self) -> Result<Self, Why> {
        if self.is_negative() {
            return Err(Why::Domain);
        }
        if let Self::Exact(r) = self {
            let (n, d) = (r.numer().sqrt(), r.denom().sqrt());
            if &(&n * &n) == r.numer() && &(&d * &d) == r.denom() {
                return Ok(Self::Exact(BigRational::new(n, d)));
            }
        }
        approx(self.to_f64().sqrt())
    }

    /// The cube root, of any sign: exact for a rational cube.
    pub(crate) fn cbrt(&self) -> Result<Self, Why> {
        if let Self::Exact(r) = self {
            let (n, d) = (r.numer().cbrt(), r.denom().cbrt());
            if &(&n * &n * &n) == r.numer() && &(&d * &d * &d) == r.denom() {
                return Ok(Self::Exact(BigRational::new(n, d)));
            }
        }
        approx(self.to_f64().cbrt())
    }

    /// Rounded by `how` (floor, ceil, half away from zero), exactly.
    pub(crate) fn rounded(&self, how: Rounding) -> Result<Self, Why> {
        match self {
            Self::Exact(r) => Ok(Self::Exact(match how {
                Rounding::Floor => r.floor(),
                Rounding::Ceil => r.ceil(),
                Rounding::Nearest => r.round(),
            })),
            Self::Approx(f) => approx(match how {
                Rounding::Floor => f.floor(),
                Rounding::Ceil => f.ceil(),
                Rounding::Nearest => f.round(),
            }),
        }
    }

    pub(crate) fn abs(&self) -> Self {
        if self.is_negative() {
            self.neg()
        } else {
            self.clone()
        }
    }

    /// The exponent `k` when this is exactly `base^k` for an integer `k` —
    /// so `log(1000)` is `3`, not `2.9999999999999996`.
    pub(crate) fn exact_log(&self, base: u32) -> Option<i64> {
        let Self::Exact(r) = self else { return None };
        if !r.is_positive() {
            return None;
        }
        let b = BigInt::from(base);
        let (mut n, mut d) = (r.numer().clone(), r.denom().clone());
        let mut k = 0i64;
        while n > BigInt::one() {
            if !(&n % &b).is_zero() {
                return None;
            }
            n /= &b;
            k += 1;
        }
        while d > BigInt::one() {
            if !(&d % &b).is_zero() {
                return None;
            }
            d /= &b;
            k -= 1;
        }
        Some(k)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Rounding {
    Floor,
    Ceil,
    Nearest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_until_it_cannot_be() {
        let tenth = exact(1, 10);
        let fifth = exact(2, 10);
        assert_eq!(tenth.add(&fifth).unwrap(), exact(3, 10), "0.1 + 0.2 is 0.3");
        assert_eq!(exact(1, 3).mul(&exact(3, 1)).unwrap(), exact(1, 1));
        assert_eq!(exact(9, 4).sqrt().unwrap(), exact(3, 2));
        assert!(!exact(2, 1).sqrt().unwrap().is_exact());
        assert_eq!(exact(-27, 8).cbrt().unwrap(), exact(-3, 2));
        assert_eq!(exact(2, 1).pow(&exact(-2, 1)).unwrap(), exact(1, 4));
        assert_eq!(exact(0, 1).pow(&exact(0, 1)).unwrap(), exact(1, 1), "0^0");
        assert_eq!(exact(7, 3).pow(&exact(0, 1)).unwrap(), exact(1, 1));
        assert_eq!(exact(0, 1).pow(&exact(3, 1)).unwrap(), exact(0, 1));
        assert_eq!(exact(1000, 1).exact_log(10), Some(3));
        assert_eq!(exact(1, 100).exact_log(10), Some(-2));
        assert_eq!(exact(5, 1).factorial().unwrap(), exact(120, 1));
        assert_eq!(exact(1, 1).div(&Number::zero()), Err(Why::DivideByZero));
        assert_eq!(exact(9, 1).pow(&exact(99_999, 1)), Err(Why::TooLarge));
        assert_eq!(exact(-8, 1).pow(&exact(1, 3)), Err(Why::Domain));
        assert_eq!(exact(-1, 1).sqrt(), Err(Why::Domain));
        assert_eq!(exact(5, 2).rounded(Rounding::Nearest).unwrap(), exact(3, 1));
        assert_eq!(
            exact(-5, 2).rounded(Rounding::Nearest).unwrap(),
            exact(-3, 1)
        );
    }
}
