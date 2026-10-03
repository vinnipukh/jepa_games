//! Difficulty tiers (D16, decided 2026-10-02): easy / medium / hard by `opt_moves`, per
//! configuration and layout, Boxoban-style. The cut points are fixed from uniform's measured
//! `opt_moves` distribution, so a tier means the same difficulty whichever generator made the
//! puzzle: a generator with a narrower range (Turan's scramble or reverse search) simply has few
//! or no puzzles in uniform's upper tiers.
//!
//! Rule: from the `opt_moves` histogram of the committed uniform stats report for the layout
//! (`reports/uniform_stats.csv`, `reports/uniform_distributed_stats.csv`, 1000 puzzles per
//! configuration), `easy_max < medium_max` are the observed values whose cumulative shares are
//! nearest to 1/3 and 2/3 (minimizing the sum of both distances; the first minimum in
//! ascending order wins). Easy is `opt_moves <= easy_max`, medium `easy_max < opt_moves <=
//! medium_max`, hard `opt_moves > medium_max`. Every tier holds at least 12 % of uniform's puzzles.
//! The CLI's report tests check this table against the committed reports.

use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::params::Params;

/// A difficulty tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Easy,
    Medium,
    Hard,
}

impl Tier {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Medium, Self::Hard];

    /// The `opt_moves` range of this tier for `(params, layout)` as `(min_opt, max_opt)`, ready
    /// for `GenConfig`; `None` outside the supported range.
    pub fn opt_band(self, params: &Params, layout: Layout) -> Option<(u32, Option<u32>)> {
        let row = tier_row(params, layout)?;
        Some(match self {
            Self::Easy => (1, Some(row.easy_max)),
            Self::Medium => (row.easy_max + 1, Some(row.medium_max)),
            Self::Hard => (row.medium_max + 1, None),
        })
    }

    /// The tier of a puzzle with `opt_moves` for `(params, layout)`; `None` outside the supported
    /// range.
    pub fn of(params: &Params, layout: Layout, opt_moves: u32) -> Option<Self> {
        let row = tier_row(params, layout)?;
        Some(if opt_moves <= row.easy_max {
            Self::Easy
        } else if opt_moves <= row.medium_max {
            Self::Medium
        } else {
            Self::Hard
        })
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Easy => "easy",
            Self::Medium => "medium",
            Self::Hard => "hard",
        })
    }
}

/// Not `easy`, `medium` or `hard`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown tier {0:?} (expected easy, medium or hard)")]
pub struct UnknownTier(pub String);

impl FromStr for Tier {
    type Err = UnknownTier;

    fn from_str(s: &str) -> Result<Self, UnknownTier> {
        match s {
            "easy" => Ok(Self::Easy),
            "medium" => Ok(Self::Medium),
            "hard" => Ok(Self::Hard),
            _ => Err(UnknownTier(s.to_owned())),
        }
    }
}

/// The tier cut points of one configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierRow {
    pub n_colors: u8,
    pub capacity: u8,
    pub n_empty: u8,
    /// Largest easy `opt_moves`.
    pub easy_max: u32,
    /// Largest medium `opt_moves`.
    pub medium_max: u32,
}

const fn tier(n_colors: u8, capacity: u8, n_empty: u8, easy_max: u32, medium_max: u32) -> TierRow {
    TierRow {
        n_colors,
        capacity,
        n_empty,
        easy_max,
        medium_max,
    }
}

/// The tier table of `layout`, one row per supported configuration.
pub const fn tier_rows(layout: Layout) -> &'static [TierRow] {
    match layout {
        Layout::Standard => TIERS_STANDARD,
        Layout::Distributed => TIERS_DISTRIBUTED,
    }
}

/// The cut points for `(params, layout)`; `None` outside the supported range.
pub fn tier_row(params: &Params, layout: Layout) -> Option<TierRow> {
    tier_rows(layout).iter().copied().find(|r| {
        (r.n_colors, r.capacity, r.n_empty) == (params.n_colors, params.capacity, params.n_empty)
    })
}

/// Standard-layout cut points, from `reports/uniform_stats.csv`.
pub const TIERS_STANDARD: &[TierRow] = &[
    tier(2, 3, 1, 3, 4),
    tier(3, 3, 1, 5, 6),
    tier(4, 3, 1, 7, 8),
    tier(5, 3, 1, 9, 10),
    tier(6, 3, 1, 11, 12),
    tier(7, 3, 1, 13, 14),
    tier(8, 3, 1, 16, 17),
    tier(9, 3, 1, 18, 19),
    tier(10, 3, 1, 20, 21),
    tier(11, 3, 1, 22, 23),
    tier(12, 3, 1, 24, 25),
    tier(2, 3, 2, 3, 4),
    tier(3, 3, 2, 5, 6),
    tier(4, 3, 2, 7, 8),
    tier(5, 3, 2, 9, 11),
    tier(6, 3, 2, 12, 13),
    tier(7, 3, 2, 14, 15),
    tier(8, 3, 2, 16, 17),
    tier(9, 3, 2, 18, 20),
    tier(10, 3, 2, 21, 22),
    tier(11, 3, 2, 23, 24),
    tier(12, 3, 2, 25, 27),
    tier(2, 4, 1, 3, 4),
    tier(3, 4, 1, 6, 8),
    tier(4, 4, 1, 10, 11),
    tier(5, 4, 1, 13, 14),
    tier(6, 4, 1, 16, 18),
    tier(7, 4, 1, 19, 21),
    tier(8, 4, 1, 23, 24),
    tier(9, 4, 1, 26, 28),
    tier(2, 4, 2, 3, 5),
    tier(3, 4, 2, 7, 8),
    tier(4, 4, 2, 10, 11),
    tier(5, 4, 2, 14, 15),
    tier(6, 4, 2, 17, 18),
    tier(7, 4, 2, 20, 21),
    tier(8, 4, 2, 23, 25),
    tier(9, 4, 2, 27, 28),
    tier(10, 4, 2, 30, 32),
    tier(11, 4, 2, 33, 35),
    tier(2, 5, 1, 4, 6),
    tier(3, 5, 1, 9, 10),
    tier(4, 5, 1, 13, 14),
    tier(5, 5, 1, 17, 19),
    tier(6, 5, 1, 21, 23),
    tier(7, 5, 1, 25, 27),
    tier(2, 5, 2, 4, 6),
    tier(3, 5, 2, 9, 10),
    tier(4, 5, 2, 13, 15),
    tier(5, 5, 2, 18, 19),
    tier(6, 5, 2, 22, 23),
    tier(7, 5, 2, 26, 28),
    tier(8, 5, 2, 30, 32),
    tier(9, 5, 2, 35, 37),
];

/// Distributed-layout cut points, from `reports/uniform_distributed_stats.csv`.
pub const TIERS_DISTRIBUTED: &[TierRow] = &[
    tier(2, 3, 1, 2, 3),
    tier(3, 3, 1, 4, 5),
    tier(4, 3, 1, 6, 7),
    tier(5, 3, 1, 8, 10),
    tier(6, 3, 1, 10, 12),
    tier(7, 3, 1, 13, 14),
    tier(8, 3, 1, 15, 16),
    tier(9, 3, 1, 17, 18),
    tier(10, 3, 1, 19, 21),
    tier(11, 3, 1, 21, 23),
    tier(12, 3, 1, 24, 25),
    tier(2, 3, 2, 2, 3),
    tier(3, 3, 2, 4, 5),
    tier(4, 3, 2, 6, 8),
    tier(5, 3, 2, 9, 10),
    tier(6, 3, 2, 11, 12),
    tier(7, 3, 2, 13, 14),
    tier(8, 3, 2, 16, 17),
    tier(9, 3, 2, 18, 19),
    tier(10, 3, 2, 20, 21),
    tier(11, 3, 2, 22, 24),
    tier(12, 3, 2, 25, 26),
    tier(2, 4, 1, 3, 4),
    tier(3, 4, 1, 6, 7),
    tier(4, 4, 1, 9, 11),
    tier(5, 4, 1, 12, 14),
    tier(6, 4, 1, 16, 17),
    tier(7, 4, 1, 19, 21),
    tier(8, 4, 1, 22, 24),
    tier(9, 4, 1, 25, 27),
    tier(2, 4, 2, 3, 4),
    tier(3, 4, 2, 6, 8),
    tier(4, 4, 2, 10, 11),
    tier(5, 4, 2, 13, 14),
    tier(6, 4, 2, 16, 17),
    tier(7, 4, 2, 19, 21),
    tier(8, 4, 2, 23, 24),
    tier(9, 4, 2, 26, 27),
    tier(10, 4, 2, 29, 31),
    tier(11, 4, 2, 32, 34),
    tier(2, 5, 1, 4, 5),
    tier(3, 5, 1, 8, 10),
    tier(4, 5, 1, 12, 14),
    tier(5, 5, 1, 17, 18),
    tier(6, 5, 1, 21, 23),
    tier(7, 5, 1, 25, 27),
    tier(8, 5, 1, 29, 31),
    tier(2, 5, 2, 4, 5),
    tier(3, 5, 2, 8, 10),
    tier(4, 5, 2, 13, 14),
    tier(5, 5, 2, 17, 18),
    tier(6, 5, 2, 21, 23),
    tier(7, 5, 2, 25, 27),
    tier(8, 5, 2, 30, 31),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supported::is_supported_in;

    #[test]
    fn rows_cover_exactly_the_supported_range() {
        for layout in Layout::ALL {
            for row in tier_rows(layout) {
                let p = Params {
                    n_colors: row.n_colors,
                    capacity: row.capacity,
                    n_empty: row.n_empty,
                };
                assert!(is_supported_in(&p, layout), "{p:?} {layout}");
                assert!(row.easy_max < row.medium_max, "{p:?} {layout}");
            }
            let mut count = 0;
            for capacity in 1..=8 {
                for n_empty in 1..=3 {
                    for n_colors in 1..=16 {
                        let p = Params {
                            n_colors,
                            capacity,
                            n_empty,
                        };
                        if is_supported_in(&p, layout) {
                            assert!(tier_row(&p, layout).is_some(), "{p:?} {layout}");
                            count += 1;
                        }
                    }
                }
            }
            assert_eq!(count, tier_rows(layout).len());
        }
    }

    #[test]
    fn bands_and_tiers_agree() {
        let p = Params {
            n_colors: 4,
            capacity: 4,
            n_empty: 2,
        };
        for layout in Layout::ALL {
            let row = tier_row(&p, layout).unwrap();
            for opt in 1..40 {
                let t = Tier::of(&p, layout, opt).unwrap();
                let (min, max) = t.opt_band(&p, layout).unwrap();
                assert!(opt >= min && max.is_none_or(|m| opt <= m), "{opt} {t}");
            }
            assert_eq!(Tier::of(&p, layout, row.easy_max), Some(Tier::Easy));
            assert_eq!(Tier::of(&p, layout, row.easy_max + 1), Some(Tier::Medium));
            assert_eq!(Tier::of(&p, layout, row.medium_max + 1), Some(Tier::Hard));
        }
        let unsupported = Params { n_empty: 3, ..p };
        assert_eq!(Tier::of(&unsupported, Layout::Standard, 5), None);
        assert_eq!(Tier::Hard.opt_band(&unsupported, Layout::Standard), None);
        for t in Tier::ALL {
            assert_eq!(t.to_string().parse::<Tier>(), Ok(t));
            assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{t}\""));
        }
        assert!("extreme".parse::<Tier>().is_err());
    }
}
