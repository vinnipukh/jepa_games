//! The supported configuration range (D3, decided 2026-10-02): the params for which generation
//! and optimal solving are fast and reliable enough to offer. The CLI, Python and the web check
//! against it.
//!
//! Only 1 or 2 empty tubes are offered ([`MAX_SUPPORTED_EMPTY`]). 0 empty tubes has no legal
//! move at all. A third empty tube barely changes `opt_moves`, while solver cost is 10–40×
//! higher, so it is left out even where it passed the measurement criterion.
//!
//! Derived from `reports/uniform_stats.md` (1000 uniform puzzles per cell, `max_states` 5e6,
//! release build). A cell is supported when every sample generated within `max_attempts`, the
//! solver p99 is below 1 s (per attempt and over accepted puzzles), the p99 time to generate one
//! puzzle (all attempts) is below 1 s, the timeout rate is below 0.1 %, and the attempts p99 is
//! at most a tenth of `max_attempts`. In every measured row the supported cells form a prefix of
//! `n_colors`.

use crate::params::Params;

/// Largest supported `n_empty` (D3).
pub const MAX_SUPPORTED_EMPTY: u8 = 2;

/// For one `(capacity, n_empty)`: `n_colors` in `min_colors..=max_colors` is supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportedRow {
    pub capacity: u8,
    pub n_empty: u8,
    pub min_colors: u8,
    pub max_colors: u8,
}

const fn row(capacity: u8, n_empty: u8, max_colors: u8) -> SupportedRow {
    SupportedRow {
        capacity,
        n_empty,
        min_colors: 2,
        max_colors,
    }
}

/// The supported range (D3). Rows not listed are unsupported.
pub const SUPPORTED: &[SupportedRow] = &[
    row(3, 1, 12),
    row(3, 2, 12),
    row(4, 1, 9),
    row(4, 2, 11),
    row(5, 1, 7),
    row(5, 2, 9),
];

/// Whether `params` lies in [`SUPPORTED`].
pub fn is_supported(params: &Params) -> bool {
    params.validate().is_ok()
        && SUPPORTED.iter().any(|r| {
            r.capacity == params.capacity
                && r.n_empty == params.n_empty
                && (r.min_colors..=r.max_colors).contains(&params.n_colors)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
        Params {
            n_colors,
            capacity,
            n_empty,
        }
    }

    #[test]
    fn lookups() {
        assert!(is_supported(&p(12, 3, 1)));
        assert!(is_supported(&p(11, 4, 2)));
        assert!(!is_supported(&p(12, 4, 2)));
        assert!(is_supported(&p(9, 4, 1)));
        assert!(!is_supported(&p(10, 4, 1)));
        assert!(
            !is_supported(&p(6, 4, 3)),
            "3 empty tubes are excluded by policy"
        );
        assert!(is_supported(&Params::with_colors(8)));
        assert!(!is_supported(&p(1, 4, 2)));
        assert!(!is_supported(&p(5, 6, 2)));
        assert!(!is_supported(&p(5, 4, 0)));
    }

    #[test]
    fn rows_are_unique_and_valid() {
        for (i, r) in SUPPORTED.iter().enumerate() {
            assert!(r.min_colors <= r.max_colors);
            assert!((1..=MAX_SUPPORTED_EMPTY).contains(&r.n_empty));
            assert_eq!(p(r.max_colors, r.capacity, r.n_empty).validate(), Ok(()));
            assert!(
                SUPPORTED[..i]
                    .iter()
                    .all(|q| (q.capacity, q.n_empty) != (r.capacity, r.n_empty))
            );
        }
    }
}
