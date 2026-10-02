//! Golden vectors: fixed inputs → committed outputs in `tests/golden/*.json`.
//!
//! The same files are re-checked by the WASM and Python test suites, so every target produces
//! bit-identical results. Regenerate with `WATER_SORT_BLESS=1 cargo test --test golden` and
//! review the diff; a change means reproducibility from old seeds is broken.

use std::path::PathBuf;

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use water_sort_core::{
    Move, Params, SolveResult, SolverLimits, State, bounded_u32, canonical_full, canonical_hash,
    fisher_yates, solve, splitmix64, time_seed,
};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

/// Compares `actual` with the committed file, or rewrites the file in bless mode.
fn check<T: Serialize + DeserializeOwned + PartialEq + core::fmt::Debug>(name: &str, actual: &T) {
    let path = golden_path(name);
    if std::env::var_os("WATER_SORT_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut json = serde_json::to_string_pretty(actual).unwrap();
        json.push('\n');
        std::fs::write(&path, json).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with WATER_SORT_BLESS=1)", path.display()));
    let expected: T = serde_json::from_str(&text).unwrap();
    assert_eq!(
        &expected, actual,
        "{name} differs from the committed golden file"
    );
}

fn hex(x: u64) -> String {
    format!("{x:016x}")
}

const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
    Params {
        n_colors,
        capacity,
        n_empty,
    }
}

const SEEDS: [u64; 6] = [0, 1, 42, 0xDEAD_BEEF, 1 << 63, u64::MAX];
const CONFIGS: [Params; 5] = [p(2, 2, 1), p(3, 3, 1), p(4, 4, 2), p(6, 4, 2), p(12, 4, 2)];

/// The first standard-layout fill the uniform generator would try for `seed`: `ChaCha20` seeded
/// from `seed`, then [`fisher_yates`] over the sorted units.
fn shuffled_units(params: Params, seed: u64) -> Vec<u8> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut units = State::sorted_units(params);
    fisher_yates(&mut rng, &mut units);
    units
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct BoundedCase {
    #[serde(with = "hex_u64")]
    seed: u64,
    n: u32,
    values: Vec<u32>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ShuffleCase {
    #[serde(with = "hex_u64")]
    seed: u64,
    params: Params,
    units: Vec<u8>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Sampling {
    rng: String,
    bounded_u32: Vec<BoundedCase>,
    fisher_yates: Vec<ShuffleCase>,
}

#[test]
fn sampling() {
    let mut bounded = Vec::new();
    for seed in SEEDS {
        for n in [1, 2, 3, 7, 10, 1000, u32::MAX] {
            let mut rng = ChaCha20Rng::seed_from_u64(seed);
            let values = (0..8).map(|_| bounded_u32(&mut rng, n)).collect();
            bounded.push(BoundedCase { seed, n, values });
        }
    }
    let shuffles = CONFIGS
        .iter()
        .flat_map(|&params| {
            SEEDS.iter().map(move |&seed| ShuffleCase {
                seed,
                params,
                units: shuffled_units(params, seed),
            })
        })
        .collect();
    check(
        "sampling.json",
        &Sampling {
            rng: "rand_chacha::ChaCha20Rng::seed_from_u64(seed)".into(),
            bounded_u32: bounded,
            fisher_yates: shuffles,
        },
    );
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct SplitmixCase {
    #[serde(with = "hex_u64")]
    input: u64,
    #[serde(with = "hex_u64")]
    output: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TimeSeedCase {
    #[serde(with = "hex_u64")]
    now_nanos: u64,
    counter: u64,
    #[serde(with = "hex_u64")]
    seed: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Seeds {
    splitmix64: Vec<SplitmixCase>,
    time_seed: Vec<TimeSeedCase>,
}

#[test]
fn seeds() {
    let splitmix = SEEDS
        .iter()
        .chain(&[2, 3, 1_759_363_200_000_000_000])
        .map(|&input| SplitmixCase {
            input,
            output: splitmix64(input),
        })
        .collect();
    let mut times = Vec::new();
    for now_nanos in [0, 1_759_363_200_000_000_000, u64::MAX] {
        for counter in [0, 1, 2, 1000] {
            times.push(TimeSeedCase {
                now_nanos,
                counter,
                seed: time_seed(now_nanos, counter),
            });
        }
    }
    check(
        "seeds.json",
        &Seeds {
            splitmix64: splitmix,
            time_seed: times,
        },
    );
}

/// States used by the canonical and solution vectors: one shuffled fill per (config, seed).
fn sample_states(configs: &[Params]) -> Vec<State> {
    configs
        .iter()
        .flat_map(|&params| {
            SEEDS
                .iter()
                .map(move |&seed| State::from_fill(params, &shuffled_units(params, seed)).unwrap())
        })
        .collect()
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct CanonicalCase {
    state: State,
    canonical: State,
    /// Hex, because JSON numbers lose precision above 2^53 in JavaScript.
    hash: String,
}

#[test]
fn canonical() {
    let mut states = sample_states(&CONFIGS);
    states.push(State::solved(p(4, 4, 2)).unwrap());
    let cases: Vec<CanonicalCase> = states
        .into_iter()
        .map(|state| CanonicalCase {
            state,
            canonical: canonical_full(&state),
            hash: hex(canonical_hash(&state)),
        })
        .collect();
    check("canonical.json", &cases);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct SolutionCase {
    state: State,
    max_states: u64,
    result: SolveResult,
}

#[test]
fn solutions() {
    let limits = SolverLimits::states(1_000_000);
    let cases: Vec<SolutionCase> = sample_states(&CONFIGS[..4])
        .into_iter()
        .map(|state| SolutionCase {
            state,
            max_states: limits.max_states,
            result: solve(&state, &limits),
        })
        .collect();
    for case in &cases {
        if let SolveResult::Solvable { solution, .. } = &case.result {
            let end = water_sort_core::replay(&case.state, solution).unwrap();
            assert!(end.is_solved());
        }
    }
    check("solutions.json", &cases);
}

#[test]
fn move_serialization_is_stable() {
    assert_eq!(
        serde_json::to_string(&Move::new(1, 2)).unwrap(),
        r#"{"from":1,"to":2}"#
    );
}

/// 64-bit values as 16-digit hex strings: JSON numbers above 2^53 are not exact in JavaScript.
mod hex_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(x: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::hex(*x))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let text = String::deserialize(d)?;
        u64::from_str_radix(&text, 16).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct PuzzleCodeCase {
    state: State,
    code: String,
}

#[test]
fn puzzle_codes() {
    let mut states = sample_states(&CONFIGS);
    states.push(State::solved(p(14, 8, 2)).unwrap());
    states.push(State::solved(p(16, 2, 0)).unwrap());
    let cases: Vec<PuzzleCodeCase> = states
        .into_iter()
        .map(|state| PuzzleCodeCase {
            state,
            code: water_sort_core::puzzle_code::encode(&state),
        })
        .collect();
    for case in &cases {
        assert_eq!(
            water_sort_core::puzzle_code::decode(&case.code),
            Ok(case.state)
        );
    }
    check("puzzle_codes.json", &cases);
}
