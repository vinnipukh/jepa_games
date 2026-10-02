//! Argument types shared by the subcommands.

use core::ops::RangeInclusive;
use core::str::FromStr;

use clap::{Args, ValueEnum};
use turan_water_sort::{Turan, TuranStrategy};
use uniform_water_sort::Uniform;
use water_sort_core::{Generator, Layout};

/// Which generator a command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GeneratorKind {
    Uniform,
    Turan,
}

/// [`Layout`] as a command-line value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LayoutArg {
    Standard,
    Distributed,
}

impl From<LayoutArg> for Layout {
    fn from(l: LayoutArg) -> Self {
        match l {
            LayoutArg::Standard => Self::Standard,
            LayoutArg::Distributed => Self::Distributed,
        }
    }
}

/// Turan strategy names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum StrategyArg {
    Scramble,
    Constrained,
}

/// A fully configured generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenSpec {
    Uniform(Uniform),
    Turan(Turan),
}

impl GenSpec {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Uniform(_) => Uniform::ID,
            Self::Turan(_) => Turan::ID,
        }
    }

    pub fn variant(self) -> String {
        match self {
            Self::Uniform(g) => g.variant(),
            Self::Turan(g) => g.variant(),
        }
    }

    pub const fn layout(self) -> Layout {
        match self {
            Self::Uniform(g) => g.layout,
            Self::Turan(g) => g.layout,
        }
    }

    /// `uniform`, `uniform_distributed`, `turan_scramble_standard`, ...: a file-name stem.
    pub fn slug(self) -> String {
        match self {
            Self::Uniform(g) => match g.layout {
                Layout::Standard => "uniform".into(),
                Layout::Distributed => "uniform_distributed".into(),
            },
            Self::Turan(g) => {
                let strategy = match g.strategy {
                    TuranStrategy::Scramble { .. } => "scramble",
                    TuranStrategy::Constrained => "constrained",
                };
                format!("turan_{strategy}_{}", g.layout)
            }
        }
    }
}

/// Generator selection flags, shared by the subcommands.
#[derive(Args, Debug, Clone)]
pub struct GenArgs {
    #[arg(long, value_enum, default_value = "uniform")]
    pub generator: GeneratorKind,
    /// Initial-state layout (D14).
    #[arg(long, value_enum, default_value = "standard")]
    pub layout: LayoutArg,
    /// Turan strategy.
    #[arg(long, value_enum, default_value = "scramble")]
    pub strategy: StrategyArg,
    /// Turan scramble: reverse pours from the solved state.
    #[arg(long, default_value_t = TuranStrategy::DEFAULT_STEPS)]
    pub steps: u32,
    /// Turan scramble, standard layout: extra pours allowed to return to the layout.
    #[arg(long, default_value_t = TuranStrategy::DEFAULT_MAX_EXTRA_STEPS)]
    pub max_extra_steps: u32,
}

impl GenArgs {
    pub fn spec(&self) -> GenSpec {
        let layout = self.layout.into();
        match self.generator {
            GeneratorKind::Uniform => GenSpec::Uniform(Uniform::new(layout)),
            GeneratorKind::Turan => GenSpec::Turan(Turan::new(
                match self.strategy {
                    StrategyArg::Scramble => TuranStrategy::Scramble {
                        steps: self.steps,
                        max_extra_steps: self.max_extra_steps,
                    },
                    StrategyArg::Constrained => TuranStrategy::Constrained,
                },
                layout,
            )),
        }
    }
}

/// `uniform`, `uniform:distributed`, `turan:scramble:40`, `turan:scramble:40:distributed`,
/// `turan:constrained:standard`, ... Fields after the generator are a layout name, a strategy
/// name, `steps` (a number) or `extra=<max_extra_steps>`, in any order.
impl FromStr for GenSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut parts = s.split(':');
        let generator = parts.next().unwrap_or_default();
        let mut layout = Layout::Standard;
        let mut strategy = None;
        let mut steps = TuranStrategy::DEFAULT_STEPS;
        let mut extra = TuranStrategy::DEFAULT_MAX_EXTRA_STEPS;
        for part in parts {
            if let Ok(l) = part.parse::<Layout>() {
                layout = l;
            } else if part == "scramble" || part == "constrained" {
                strategy = Some(part);
            } else if let Ok(n) = part.parse::<u32>() {
                steps = n;
            } else if let Some(n) = part.strip_prefix("extra=") {
                extra = n
                    .parse()
                    .map_err(|e| format!("invalid extra in {s:?}: {e}"))?;
            } else {
                return Err(format!("unknown field {part:?} in generator spec {s:?}"));
            }
        }
        match generator {
            "uniform" if strategy.is_none() => Ok(Self::Uniform(Uniform::new(layout))),
            "turan" => Ok(Self::Turan(Turan::new(
                if strategy == Some("constrained") {
                    TuranStrategy::Constrained
                } else {
                    TuranStrategy::Scramble {
                        steps,
                        max_extra_steps: extra,
                    }
                },
                layout,
            ))),
            _ => Err(format!("invalid generator spec {s:?}")),
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
    fn specs() {
        let spec = |s: &str| s.parse::<GenSpec>().map(GenSpec::variant);
        assert_eq!(spec("uniform"), Ok("fisher_yates".into()));
        assert_eq!(
            spec("uniform:distributed"),
            Ok("fisher_yates(layout=distributed)".into())
        );
        assert_eq!(
            spec("turan"),
            Ok("scramble(steps=40,max_extra_steps=100,layout=standard)".into())
        );
        assert_eq!(
            spec("turan:scramble:80:distributed"),
            Ok("scramble(steps=80,layout=distributed)".into())
        );
        assert_eq!(
            spec("turan:20:extra=5"),
            Ok("scramble(steps=20,max_extra_steps=5,layout=standard)".into())
        );
        assert_eq!(
            spec("turan:constrained:distributed"),
            Ok("constrained(layout=distributed)".into())
        );
        assert!(spec("uniform:constrained").is_err());
        assert!(spec("turan:sideways").is_err());
        assert!(spec("other").is_err());
        let slug = |s: &str| s.parse::<GenSpec>().unwrap().slug();
        assert_eq!(slug("uniform"), "uniform");
        assert_eq!(slug("uniform:distributed"), "uniform_distributed");
        assert_eq!(slug("turan:distributed"), "turan_scramble_distributed");
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
