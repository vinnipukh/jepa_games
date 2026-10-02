//! Argument types shared by the subcommands.

use core::ops::RangeInclusive;
use core::str::FromStr;

use clap::ValueEnum;

/// Which generator a command runs. Turan plugs in here as a new arm (Phase 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GeneratorKind {
    Uniform,
}

impl GeneratorKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
        }
    }
}

/// An inclusive integer range: `a..=b`, `a..b` (exclusive end) or a single value `a`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range(pub RangeInclusive<u8>);

impl FromStr for Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let num = |t: &str| {
            t.trim()
                .parse::<u8>()
                .map_err(|e| format!("invalid number {t:?}: {e}"))
        };
        let range = if let Some((a, b)) = s.split_once("..=") {
            num(a)?..=num(b)?
        } else if let Some((a, b)) = s.split_once("..") {
            let end = num(b)?
                .checked_sub(1)
                .ok_or_else(|| format!("empty range {s:?}"))?;
            num(a)?..=end
        } else {
            let v = num(s)?;
            v..=v
        };
        if range.is_empty() {
            return Err(format!("empty range {s:?}"));
        }
        Ok(Self(range))
    }
}

/// A count that also accepts scientific notation, e.g. `5e6`.
pub fn parse_count(s: &str) -> Result<u64, String> {
    if let Ok(v) = s.parse::<u64>() {
        return Ok(v);
    }
    let (mantissa, exp) = s
        .split_once(['e', 'E'])
        .ok_or_else(|| format!("invalid count {s:?}"))?;
    let mantissa: u64 = mantissa
        .parse()
        .map_err(|e| format!("invalid count {s:?}: {e}"))?;
    let exp: u32 = exp
        .parse()
        .map_err(|e| format!("invalid count {s:?}: {e}"))?;
    10u64
        .checked_pow(exp)
        .and_then(|p| p.checked_mul(mantissa))
        .ok_or_else(|| format!("count {s:?} overflows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!("2..=12".parse(), Ok(Range(2..=12)));
        assert_eq!("3..6".parse(), Ok(Range(3..=5)));
        assert_eq!("4".parse(), Ok(Range(4..=4)));
        assert!("5..=3".parse::<Range>().is_err());
        assert!("3..3".parse::<Range>().is_err());
        assert!("0..0".parse::<Range>().is_err());
        assert!("x..=3".parse::<Range>().is_err());
    }

    #[test]
    fn counts() {
        assert_eq!(parse_count("5000000"), Ok(5_000_000));
        assert_eq!(parse_count("5e6"), Ok(5_000_000));
        assert_eq!(parse_count("1E3"), Ok(1000));
        assert!(parse_count("5e30").is_err());
        assert!(parse_count("1.5e6").is_err());
        assert!(parse_count("abc").is_err());
    }
}
