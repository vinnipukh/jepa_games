//! Shared helpers for the integration tests: random states and proptest strategies.

#![allow(dead_code)]

use proptest::prelude::*;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};
use water_sort_core::{Params, State};

pub fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// Uniform index in `0..n` (test-only; modulo bias is irrelevant here).
pub fn below(rng: &mut ChaCha20Rng, n: usize) -> usize {
    usize::try_from(rng.next_u64() % n as u64).unwrap()
}

fn shuffle(rng: &mut ChaCha20Rng, xs: &mut [u8]) {
    for i in (1..xs.len()).rev() {
        xs.swap(i, below(rng, i + 1));
    }
}

/// A random standard-layout state: full tubes first, then `n_empty` empty tubes.
pub fn random_fill(params: Params, seed: u64) -> State {
    let mut rng = rng(seed);
    let mut units = State::sorted_units(params);
    shuffle(&mut rng, &mut units);
    State::from_fill(params, &units).unwrap()
}

/// A random state with arbitrary tube heights: each unit goes onto a random non-full tube.
pub fn random_state(params: Params, seed: u64) -> State {
    let mut rng = rng(seed);
    let mut units = State::sorted_units(params);
    shuffle(&mut rng, &mut units);
    let mut tubes = vec![Vec::new(); params.n_tubes()];
    for u in units {
        let open: Vec<usize> = (0..tubes.len())
            .filter(|&i| tubes[i].len() < usize::from(params.capacity))
            .collect();
        tubes[open[below(&mut rng, open.len())]].push(u);
    }
    State::from_tubes(params, &tubes).unwrap()
}

/// Applies a tube permutation and a color relabeling to `s`.
pub fn permute(s: &State, tube_perm: &[usize], color_perm: &[u8]) -> State {
    let tubes = s.tubes();
    let permuted: Vec<Vec<u8>> = tube_perm
        .iter()
        .map(|&i| {
            tubes[i]
                .iter()
                .map(|&c| color_perm[usize::from(c)])
                .collect()
        })
        .collect();
    State::from_tubes(s.params(), &permuted).unwrap()
}

/// Per-color unit counts.
pub fn color_counts(s: &State) -> Vec<usize> {
    let mut counts = vec![0; usize::from(s.params().n_colors)];
    for t in s.tubes() {
        for c in t {
            counts[usize::from(c)] += 1;
        }
    }
    counts
}

pub fn params(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
    Params {
        n_colors,
        capacity,
        n_empty,
    }
}

/// Params with `n_colors` in `colors`, `capacity` in `caps`, `n_empty` in `empties`.
pub fn params_in(
    colors: core::ops::RangeInclusive<u8>,
    caps: core::ops::RangeInclusive<u8>,
    empties: core::ops::RangeInclusive<u8>,
) -> impl Strategy<Value = Params> {
    (colors, caps, empties).prop_map(|(c, k, e)| params(c, k, e))
}

/// Random states (arbitrary heights) for moderately sized params.
pub fn any_state() -> impl Strategy<Value = State> {
    (params_in(1..=6, 2..=5, 0..=3), any::<u64>()).prop_map(|(p, seed)| random_state(p, seed))
}
