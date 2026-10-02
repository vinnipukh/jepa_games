//! Uniformity validation (Phase 2.2): accepted puzzles are uniform over the accepted labeled set.
//!
//! For tiny params, every standard-layout fill is enumerated and filtered with the generator's
//! own [`evaluate`] and config, giving the accepted set `A`. The generator is sampled
//! `50 * |A|` times from a fixed seed list, so the test is deterministic and never flakes, and a
//! Pearson chi-square test with `|A| - 1` degrees of freedom must give `p > 0.001`. A shuffle
//! with a deliberate off-by-one bug must fail the same test, which shows the test has power.
//!
//! The 3-color tests are `#[ignore]` and run in the release-mode heavy-tests CI job.

use std::collections::HashMap;

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use uniform_water_sort::Uniform;
use water_sort_core::{
    GenConfig, Generator, MetricsConfig, Params, State, attempt_loop, bounded_u32, evaluate,
    is_symmetric, splitmix64, standard_fills,
};

const SAMPLES_PER_STATE: usize = 50;
const ALPHA: f64 = 0.001;

/// Metrics do not affect acceptance; skipping the rollouts only makes sampling faster.
const CFG: GenConfig = GenConfig {
    min_opt: 1,
    max_attempts: 10_000,
    max_states: 5_000_000,
    metrics: MetricsConfig {
        random_rollouts: 0,
        rollout_cap_factor: 4,
        dead_end_ratios: false,
    },
};

const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
    Params {
        n_colors,
        capacity,
        n_empty,
    }
}

/// The fixed seed list: `splitmix64(SALT ^ i)`.
fn seed(i: usize) -> u64 {
    const SALT: u64 = 0x0C41_5A4E_D7E5_7000;
    splitmix64(SALT ^ i as u64)
}

/// The accepted set `A`, indexed.
fn accepted_set(params: Params) -> HashMap<State, usize> {
    standard_fills(params)
        .unwrap()
        .filter(|s| evaluate(s, &CFG).is_ok())
        .enumerate()
        .map(|(i, s)| (s, i))
        .collect()
}

struct ChiSquare {
    statistic: f64,
    df: f64,
    p_value: f64,
}

/// Pearson chi-square of `counts` against the uniform distribution.
#[allow(clippy::cast_precision_loss)] // counts are far below 2^52
fn chi_square(counts: &[u64]) -> ChiSquare {
    let n: u64 = counts.iter().sum();
    let expected = n as f64 / counts.len() as f64;
    let statistic = counts
        .iter()
        .map(|&o| (o as f64 - expected).powi(2) / expected)
        .sum();
    let df = (counts.len() - 1) as f64;
    let p_value = ChiSquared::new(df).unwrap().sf(statistic);
    ChiSquare {
        statistic,
        df,
        p_value,
    }
}

/// Samples `50 * |A|` puzzles with `sample(seed)` and tests them for uniformity over `A`.
fn test_uniformity(params: Params, sample: impl Fn(u64) -> State) -> ChiSquare {
    let set = accepted_set(params);
    assert!(set.len() > 1, "{params:?}: accepted set too small to test");
    let mut counts = vec![0u64; set.len()];
    for i in 0..SAMPLES_PER_STATE * set.len() {
        let state = sample(seed(i));
        let idx = set
            .get(&state)
            .unwrap_or_else(|| panic!("sample {state} is not in the accepted set"));
        counts[*idx] += 1;
    }
    let result = chi_square(&counts);
    let symmetric = set.keys().filter(|s| is_symmetric(s)).count();
    println!(
        "{params:?}: |A| = {}, symmetric = {symmetric}, chi2 = {:.1}, df = {}, p = {:.4}",
        set.len(),
        result.statistic,
        result.df,
        result.p_value
    );
    result
}

fn uniform(params: Params) -> impl Fn(u64) -> State {
    move |seed| {
        Uniform::default()
            .generate(&params, seed, &CFG)
            .unwrap()
            .state
    }
}

/// The uniform generator with a buggy shuffle in place of Fisher-Yates.
fn biased(params: Params, shuffle: fn(&mut ChaCha20Rng, &mut [u8])) -> impl Fn(u64) -> State {
    move |seed| {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let (state, _, _) = attempt_loop(&CFG, &mut (), || {
            let mut units = State::sorted_units(params);
            shuffle(&mut rng, &mut units);
            State::from_fill(params, &units).unwrap()
        })
        .unwrap();
        state
    }
}

/// Off by one in the bound: `j` is drawn from `0..i` instead of `0..=i` (Sattolo's algorithm,
/// which only produces cyclic permutations of the units).
fn sattolo<R: Rng>(rng: &mut R, xs: &mut [u8]) {
    for i in (1..xs.len()).rev() {
        let j = bounded_u32(rng, u32::try_from(i).unwrap()) as usize;
        xs.swap(i, j);
    }
}

/// Off by one in the loop start: `i` starts at `len - 2`, so the last unit never moves.
///
/// Needed for the 2-color smoke test, whose accepted set is so symmetric that [`sattolo`] still
/// hits it uniformly.
fn last_unit_fixed<R: Rng>(rng: &mut R, xs: &mut [u8]) {
    for i in (1..xs.len() - 1).rev() {
        let j = bounded_u32(rng, u32::try_from(i + 1).unwrap()) as usize;
        xs.swap(i, j);
    }
}

#[test]
fn chi_square_reference_values() {
    // Equal counts give a statistic of 0 and p = 1.
    let flat = chi_square(&[50; 10]);
    assert!(flat.statistic.abs() < 1e-12 && (flat.p_value - 1.0).abs() < 1e-12);
    // chi2 = 2 * 50^2 / 100 = 50, df = 1: p is about 1.5e-12.
    let skewed = chi_square(&[150, 50]);
    assert!((skewed.statistic - 50.0).abs() < 1e-9);
    assert!(skewed.p_value < 1e-11);
    // The 0.999 quantile of chi-square(10) is 29.588.
    let at_quantile = ChiSquared::new(10.0).unwrap().sf(29.588);
    assert!((at_quantile - 0.001).abs() < 1e-5, "{at_quantile}");
}

#[test]
fn uniform_2x2_1() {
    let params = p(2, 2, 1);
    assert_eq!(accepted_set(params).len(), 4);
    let result = test_uniformity(params, uniform(params));
    assert!(result.p_value > ALPHA, "p = {}", result.p_value);
}

#[test]
fn biased_2x2_1_fails() {
    let params = p(2, 2, 1);
    let result = test_uniformity(params, biased(params, last_unit_fixed));
    assert!(result.p_value < ALPHA, "p = {}", result.p_value);
}

#[test]
#[ignore = "release-mode heavy test: 50 * |A| generations"]
fn uniform_3x3_1() {
    let params = p(3, 3, 1);
    let result = test_uniformity(params, uniform(params));
    assert!(result.p_value > ALPHA, "p = {}", result.p_value);
}

#[test]
#[ignore = "release-mode heavy test: 50 * |A| generations"]
fn biased_3x3_1_fails() {
    let params = p(3, 3, 1);
    let result = test_uniformity(params, biased(params, sattolo));
    assert!(result.p_value < ALPHA, "p = {}", result.p_value);
}
