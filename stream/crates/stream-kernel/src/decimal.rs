//! Exact decimals for quantities. Never floating point.

use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;

/// `mant × 10^-scale`, exact.
#[derive(Clone, Copy, Debug)]
pub struct Decimal {
    mant: i128,
    scale: u32,
}

pub const MAX_SCALE: u32 = 18;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecimalError;

impl Decimal {
    pub const ZERO: Decimal = Decimal { mant: 0, scale: 0 };

    /// `mant × 10^-scale`. Scale is capped at [`MAX_SCALE`].
    pub fn from_parts(mant: i128, scale: u32) -> Decimal {
        Decimal { mant, scale: scale.min(MAX_SCALE) }
    }

    /// Lossy conversion for ratios and display only; never for the Books.
    pub fn to_f64(self) -> f64 {
        let mut d = 1f64;
        for _ in 0..self.scale {
            d *= 10.0;
        }
        self.mant as f64 / d
    }

    /// Parse `[-]digits[.digits]`. An estimate marker `~` is not accepted
    /// here; use [`Decimal::parse_marked`].
    pub fn parse(s: &str) -> Result<Decimal, DecimalError> {
        let (neg, body) = match s.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, s),
        };
        let (int, frac) = match body.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (body, None),
        };
        if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DecimalError);
        }
        let frac = frac.unwrap_or("");
        if body.contains('.') && frac.is_empty() {
            return Err(DecimalError);
        }
        if !frac.bytes().all(|b| b.is_ascii_digit()) || frac.len() as u32 > MAX_SCALE {
            return Err(DecimalError);
        }
        let mut mant: i128 = 0;
        for b in int.bytes().chain(frac.bytes()) {
            mant = mant
                .checked_mul(10)
                .and_then(|m| m.checked_add((b - b'0') as i128))
                .ok_or(DecimalError)?;
        }
        Ok(Decimal { mant: if neg { -mant } else { mant }, scale: frac.len() as u32 })
    }

    /// Parse a value that may carry the estimate marker `~`.
    pub fn parse_marked(s: &str) -> Result<(Decimal, bool), DecimalError> {
        match s.strip_prefix('~') {
            Some(r) => Ok((Decimal::parse(r)?, true)),
            None => Ok((Decimal::parse(s)?, false)),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.mant == 0
    }

    fn rescale(&self, scale: u32) -> Option<i128> {
        self.mant.checked_mul(10i128.checked_pow(scale - self.scale)?)
    }

    pub fn checked_add(self, o: Decimal) -> Option<Decimal> {
        let s = self.scale.max(o.scale);
        Some(Decimal { mant: self.rescale(s)?.checked_add(o.rescale(s)?)?, scale: s })
    }

    pub fn checked_sub(self, o: Decimal) -> Option<Decimal> {
        self.checked_add(-o)
    }

    /// Multiply (used by priced views). Result scale is the sum of scales,
    /// capped by normalisation.
    pub fn checked_mul(self, o: Decimal) -> Option<Decimal> {
        let d = Decimal { mant: self.mant.checked_mul(o.mant)?, scale: self.scale + o.scale };
        Some(d.normalized())
    }

    /// Strip trailing fractional zeros.
    pub fn normalized(self) -> Decimal {
        let (mut m, mut s) = (self.mant, self.scale);
        while s > 0 && m % 10 == 0 {
            m /= 10;
            s -= 1;
        }
        Decimal { mant: m, scale: s }
    }
}

impl core::ops::Neg for Decimal {
    type Output = Decimal;
    fn neg(self) -> Decimal {
        Decimal { mant: -self.mant, scale: self.scale }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, o: &Decimal) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Decimal {}
impl PartialOrd for Decimal {
    fn partial_cmp(&self, o: &Decimal) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Decimal {
    fn cmp(&self, o: &Decimal) -> Ordering {
        let s = self.scale.max(o.scale);
        match (self.rescale(s), o.rescale(s)) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => self.mant.signum().cmp(&o.mant.signum()),
        }
    }
}

impl fmt::Display for Decimal {
    /// Normalised form: no trailing fractional zeros, "0" for zero.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.normalized();
        let neg = d.mant < 0;
        let digits = alloc::format!("{}", d.mant.unsigned_abs());
        let s = d.scale as usize;
        let mut out = String::new();
        if neg {
            out.push('-');
        }
        if s == 0 {
            out.push_str(&digits);
        } else if digits.len() > s {
            out.push_str(&digits[..digits.len() - s]);
            out.push('.');
            out.push_str(&digits[digits.len() - s..]);
        } else {
            out.push_str("0.");
            for _ in 0..(s - digits.len()) {
                out.push('0');
            }
            out.push_str(&digits);
        }
        f.write_str(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn parse_add_display() {
        let a = Decimal::parse("0.019").unwrap();
        let b = Decimal::parse("-0.019").unwrap();
        assert!(a.checked_add(b).unwrap().is_zero());
        assert_eq!(Decimal::parse("52.40").unwrap().to_string(), "52.4");
        assert_eq!(Decimal::parse("-0.05").unwrap().to_string(), "-0.05");
        assert_eq!(Decimal::parse("92").unwrap(), Decimal::parse("92.000").unwrap());
        assert!(Decimal::parse_marked("~480").unwrap().1);
        for bad in ["", "1.", ".5", "+1", "1e3", "--1", "1.2.3", "NaN"] {
            assert!(Decimal::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn multiply_prices() {
        let q = Decimal::parse("1200").unwrap();
        let r = Decimal::parse("0.000003").unwrap();
        assert_eq!(q.checked_mul(r).unwrap().to_string(), "0.0036");
    }
}
