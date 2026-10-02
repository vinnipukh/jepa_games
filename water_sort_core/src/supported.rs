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

use crate::layout::Layout;
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

/// The supported range for [`Layout::Distributed`] (D3 addendum, **proposed** in Phase 3; the
/// user decides). Measured with `reports/uniform_distributed_stats.md` (cloud machine, timings
/// scaled by the 1.28× calibration factor against the desktop that measured [`SUPPORTED`], see
/// `reports/uniform_standard_calibration_cloud.csv`). Spreading the free space makes fewer
/// fills unsolvable (one more color at capacity 5 with one empty tube), but costs the solver
/// more with two empty tubes (one color fewer at capacity 5).
pub const SUPPORTED_DISTRIBUTED: &[SupportedRow] = &[
    row(3, 1, 12),
    row(3, 2, 12),
    row(4, 1, 9),
    row(4, 2, 11),
    row(5, 1, 8),
    row(5, 2, 8),
];

/// The supported rows of `layout`.
pub const fn supported_rows(layout: Layout) -> &'static [SupportedRow] {
    match layout {
        Layout::Standard => SUPPORTED,
        Layout::Distributed => SUPPORTED_DISTRIBUTED,
    }
}

/// Whether `params` lies in the standard-layout range [`SUPPORTED`]. Same as
/// `is_supported_in(params, Layout::Standard)`.
pub fn is_supported(params: &Params) -> bool {
    is_supported_in(params, Layout::Standard)
}

/// Whether `params` is supported for `layout` ([`supported_rows`]).
pub fn is_supported_in(params: &Params, layout: Layout) -> bool {
    params.validate().is_ok()
        && supported_rows(layout).iter().any(|r| {
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
    fn layout_lookups() {
        let d = Layout::Distributed;
        assert!(is_supported_in(&p(8, 5, 1), d));
        assert!(!is_supported_in(&p(8, 5, 1), Layout::Standard));
        assert!(!is_supported_in(&p(9, 5, 2), d));
        assert!(is_supported_in(&p(11, 4, 2), d));
        assert!(!is_supported_in(&p(6, 4, 3), d));
        assert_eq!(
            is_supported(&p(9, 5, 2)),
            is_supported_in(&p(9, 5, 2), Layout::Standard)
        );
    }

    #[test]
    fn rows_are_unique_and_valid() {
        for (i, r) in SUPPORTED_DISTRIBUTED.iter().enumerate() {
            assert!(
                SUPPORTED_DISTRIBUTED[..i]
                    .iter()
                    .all(|q| (q.capacity, q.n_empty) != (r.capacity, r.n_empty))
            );
            assert!((1..=MAX_SUPPORTED_EMPTY).contains(&r.n_empty));
        }
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
