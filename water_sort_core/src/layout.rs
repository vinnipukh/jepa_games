//! Initial-state layouts (D14): where the `n_empty × capacity` free slots sit.
//!
//! Every color has exactly `capacity` units, so the free space is always a whole number of
//! tubes. [`Layout::Standard`] keeps it in whole empty tubes after the full ones;
//! [`Layout::Distributed`] spreads it over the tubes (half-empty tubes allowed).

use core::fmt;
use core::str::FromStr;

use rand_core::Rng;
use serde::{Deserialize, Serialize};

use crate::params::{MAX_CAP, MAX_TUBES, Params};
use crate::sampling::bounded_u64;
use crate::state::{State, to_u8};

/// How the free space of an initial state is laid out.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// The first `n_colors` tubes full, the last `n_empty` tubes empty (the classic game).
    #[default]
    Standard,
    /// Any fill heights `h_i ∈ [0, capacity]` with `Σ h_i = n_colors × capacity`, units packed
    /// from the bottom. Includes the standard layout's height vectors as special cases.
    Distributed,
}

impl Layout {
    pub const ALL: [Self; 2] = [Self::Standard, Self::Distributed];

    /// Lower-case name, as used in `variant()` strings and on the command line.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Distributed => "distributed",
        }
    }

    /// Whether `state` has this layout.
    ///
    /// Every valid [`State`] is packed from the bottom and holds `n_colors × capacity` units, so
    /// every state is `Distributed`; only `Standard` constrains the heights.
    pub fn matches(self, state: &State) -> bool {
        match self {
            Self::Standard => {
                let n_colors = usize::from(state.params().n_colors);
                (0..state.n_tubes()).all(|i| {
                    if i < n_colors {
                        state.is_tube_full(i)
                    } else {
                        state.is_tube_empty(i)
                    }
                })
            }
            Self::Distributed => true,
        }
    }
}

impl fmt::Display for Layout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// An unknown layout name.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown layout {0:?} (expected \"standard\" or \"distributed\")")]
pub struct UnknownLayout(pub String);

impl FromStr for Layout {
    type Err = UnknownLayout;

    fn from_str(s: &str) -> Result<Self, UnknownLayout> {
        Self::ALL
            .into_iter()
            .find(|l| l.name() == s)
            .ok_or_else(|| UnknownLayout(s.to_owned()))
    }
}

impl State {
    /// Whether this state has `layout`; see [`Layout::matches`].
    pub fn layout_matches(&self, layout: Layout) -> bool {
        layout.matches(self)
    }
}

/// Completion counts for distributed height vectors: `counts[i][r]` is the number of ways tubes
/// `i..n_tubes` can hold exactly `r` units, each tube between 0 and `capacity`.
///
/// The largest count is at most `(MAX_CAP + 1)^MAX_TUBES = 9^16 < 2^51`, so `u64` is exact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeightCounts {
    params: Params,
    counts: Vec<Vec<u64>>,
}

impl HeightCounts {
    /// The DP table for `params` (assumed valid).
    pub fn new(params: Params) -> Self {
        let n = params.n_tubes();
        let cap = usize::from(params.capacity);
        let units = params.n_units();
        debug_assert!(n <= MAX_TUBES && cap <= MAX_CAP);
        let mut counts = vec![vec![0u64; units + 1]; n + 1];
        counts[n][0] = 1;
        for i in (0..n).rev() {
            for r in 0..=units {
                counts[i][r] = (0..=cap.min(r)).map(|h| counts[i + 1][r - h]).sum();
            }
        }
        Self { params, counts }
    }

    /// Number of valid distributed height vectors.
    pub fn total(&self) -> u64 {
        self.counts[0][self.params.n_units()]
    }

    /// Draws a height vector exactly uniformly: tube by tube, height `h` with probability
    /// proportional to the number of completions of the remaining tubes.
    pub fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec<u8> {
        let n = self.params.n_tubes();
        let cap = usize::from(self.params.capacity);
        let mut left = self.params.n_units();
        let mut heights = Vec::with_capacity(n);
        for i in 0..n {
            let mut x = bounded_u64(rng, self.counts[i][left]);
            let mut h = 0;
            loop {
                let ways = self.counts[i + 1][left - h];
                if x < ways {
                    break;
                }
                x -= ways;
                h += 1;
            }
            debug_assert!(h <= cap);
            heights.push(to_u8(h));
            left -= h;
        }
        debug_assert_eq!(left, 0);
        heights
    }
}

/// One exactly uniform distributed height vector (see [`HeightCounts::sample`]). Builds the DP
/// table on every call; generators that sample repeatedly keep a [`HeightCounts`].
pub fn sample_heights<R: Rng + ?Sized>(rng: &mut R, params: Params) -> Vec<u8> {
    HeightCounts::new(params).sample(rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateError;

    const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
        Params {
            n_colors,
            capacity,
            n_empty,
        }
    }

    /// Brute force: all vectors in `[0, cap]^n` with the right sum.
    fn brute_force(params: Params) -> Vec<Vec<u8>> {
        let n = params.n_tubes();
        let cap = params.capacity;
        let mut out = Vec::new();
        let mut h = vec![0u8; n];
        loop {
            if h.iter().map(|&x| usize::from(x)).sum::<usize>() == params.n_units() {
                out.push(h.clone());
            }
            let Some(i) = (0..n).rev().find(|&i| h[i] < cap) else {
                return out;
            };
            h[i] += 1;
            h[i + 1..].fill(0);
        }
    }

    #[test]
    fn counts_match_brute_force() {
        for params in [
            p(1, 2, 1),
            p(2, 2, 1),
            p(3, 3, 1),
            p(3, 4, 2),
            p(4, 3, 2),
            p(5, 2, 2),
        ] {
            assert_eq!(
                HeightCounts::new(params).total(),
                brute_force(params).len() as u64,
                "{params:?}"
            );
        }
        assert_eq!(HeightCounts::new(p(2, 2, 1)).total(), 6);
        assert_eq!(HeightCounts::new(p(3, 3, 1)).total(), 20);
        // Zero free space: only the full vector.
        assert_eq!(HeightCounts::new(p(3, 4, 0)).total(), 1);
    }

    #[test]
    fn counts_fit_at_the_limits() {
        // 16 tubes of capacity 8: the largest table core allows.
        for n_empty in 0..=15 {
            let params = p(16 - n_empty, 8, n_empty);
            let total = HeightCounts::new(params).total();
            assert!(total >= 1 && total < 9u64.pow(16), "{params:?}");
        }
    }

    #[test]
    fn samples_are_valid_and_cover_everything() {
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;
        let params = p(3, 3, 1);
        let all = brute_force(params);
        let table = HeightCounts::new(params);
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..2000 {
            let h = table.sample(&mut rng);
            assert!(all.contains(&h), "{h:?}");
            seen.insert(h);
        }
        assert_eq!(seen.len(), all.len());
    }

    /// Pearson chi-square of `samples` draws over all height vectors of `params`.
    #[allow(clippy::cast_precision_loss)]
    fn chi_square(params: Params, samples: usize, mut draw: impl FnMut() -> Vec<u8>) -> f64 {
        let all = brute_force(params);
        let mut counts = vec![0u64; all.len()];
        for _ in 0..samples {
            let h = draw();
            counts[all.iter().position(|v| *v == h).unwrap()] += 1;
        }
        let expected = samples as f64 / all.len() as f64;
        counts
            .iter()
            .map(|&o| (o as f64 - expected).powi(2) / expected)
            .sum()
    }

    #[test]
    fn sampling_is_uniform_with_a_failing_control() {
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;
        // 3 x 3 x 1 has 20 height vectors; the 0.999 quantile of chi-square(19) is 43.82.
        const CRITICAL: f64 = 43.82;
        let params = p(3, 3, 1);
        let table = HeightCounts::new(params);
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        let exact = chi_square(params, 20_000, || table.sample(&mut rng));
        assert!(exact < CRITICAL, "chi2 = {exact}");
        // Control: each tube uniform over its feasible range given the units left. Biased.
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        let biased = chi_square(params, 20_000, || {
            let (n, cap) = (params.n_tubes(), usize::from(params.capacity));
            let mut left = params.n_units();
            let mut h = Vec::new();
            for i in 0..n {
                let lo = left.saturating_sub(cap * (n - i - 1));
                let hi = cap.min(left);
                let span = u64::try_from(hi - lo + 1).unwrap();
                let v = lo + usize::try_from(bounded_u64(&mut rng, span)).unwrap();
                h.push(to_u8(v));
                left -= v;
            }
            h
        });
        assert!(biased > CRITICAL, "chi2 = {biased}");
    }

    #[test]
    fn puzzle_codes_round_trip_distributed_states() {
        for params in [p(2, 2, 1), p(2, 3, 2)] {
            for s in crate::enumerate::distributed_fills(params).unwrap() {
                assert_eq!(
                    crate::puzzle_code::decode(&crate::puzzle_code::encode(&s)),
                    Ok(s)
                );
            }
        }
    }

    #[test]
    fn layout_checks() {
        let params = p(3, 3, 1);
        let solved = State::solved(params).unwrap();
        assert!(Layout::Standard.matches(&solved));
        assert!(solved.layout_matches(Layout::Distributed));
        let spread =
            State::from_heights(params, &[3, 2, 2, 2], &State::sorted_units(params)).unwrap();
        assert_eq!(
            spread.tubes(),
            vec![vec![0, 0, 0], vec![1, 1], vec![1, 2], vec![2, 2]]
        );
        assert!(!Layout::Standard.matches(&spread));
        assert!(Layout::Distributed.matches(&spread));
        // Full tubes in the wrong place are not standard either.
        let moved =
            State::from_tubes(params, &[&[][..], &[0, 0, 0], &[1, 1, 1], &[2, 2, 2]]).unwrap();
        assert!(!Layout::Standard.matches(&moved));
    }

    #[test]
    fn names_round_trip() {
        for layout in Layout::ALL {
            assert_eq!(layout.name().parse(), Ok(layout));
            assert_eq!(layout.to_string(), layout.name());
            assert_eq!(
                serde_json::to_string(&layout).unwrap(),
                format!("\"{}\"", layout.name())
            );
        }
        assert!("half".parse::<Layout>().is_err());
        assert_eq!(Layout::default(), Layout::Standard);
    }

    #[test]
    fn from_heights_validation() {
        let params = p(2, 2, 1);
        let units = State::sorted_units(params);
        assert_eq!(
            State::from_heights(params, &[2, 2], &units),
            Err(StateError::TubeCount {
                expected: 3,
                got: 2
            })
        );
        assert_eq!(
            State::from_heights(params, &[3, 1, 0], &units),
            Err(StateError::TubeOverflow {
                tube: 0,
                len: 3,
                capacity: 2
            })
        );
        assert_eq!(
            State::from_heights(params, &[1, 1, 1], &units),
            Err(StateError::UnitCount {
                expected: 4,
                got: 3
            })
        );
        assert_eq!(
            State::from_heights(params, &[2, 2, 0], &units),
            State::from_fill(params, &units)
        );
    }
}
