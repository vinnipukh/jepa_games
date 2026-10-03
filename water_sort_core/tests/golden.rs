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
    EpisodeRules, Move, Params, SolveResult, SolverLimits, State, bounded_u32, canonical_full,
    canonical_hash, episode_step, fisher_yates, legal_moves, move_limit, puzzle_code,
    sample_heights, solve, splitmix64, time_seed,
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

// The flags mirror the Gymnasium step outputs.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RolloutStep {
    action: usize,
    /// Puzzle code of the state after the step.
    state: String,
    units_moved: u8,
    reward: f64,
    illegal: bool,
    dead_end: bool,
    solved: bool,
    terminated: bool,
    truncated: bool,
    moves_so_far: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RolloutCase {
    name: String,
    puzzle_code: String,
    /// `None` for an unsolvable start (replayed with `env_step`, not through the env).
    opt_moves: Option<u32>,
    move_limit_k: u32,
    move_limit: u32,
    shaping_gamma: Option<f64>,
    dead_end_max_states: Option<u64>,
    steps: Vec<RolloutStep>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Rollouts {
    rules: String,
    cases: Vec<RolloutCase>,
}

/// Plays `actions` (or, with `policy_seed`, a ChaCha-driven mix of 70 % random legal and 30 %
/// arbitrary actions until the episode ends) under the episode rules.
fn rollout(
    name: String,
    start: State,
    opt_moves: Option<u32>,
    k: u32,
    rules: EpisodeRules,
    actions: Option<&[usize]>,
    policy_seed: u64,
) -> RolloutCase {
    let n_actions = u32::try_from(start.n_tubes() * start.n_tubes()).unwrap();
    let mut rng = ChaCha20Rng::seed_from_u64(policy_seed);
    let mut state = start;
    let mut moves = 0;
    let mut steps = Vec::new();
    for i in 0..400 {
        let action = match actions {
            Some(list) if i < list.len() => list[i],
            Some(_) => break,
            None => {
                let legal: Vec<Move> = legal_moves(&state).collect();
                if legal.is_empty() || bounded_u32(&mut rng, 10) < 3 {
                    bounded_u32(&mut rng, n_actions) as usize
                } else {
                    let j = bounded_u32(&mut rng, u32::try_from(legal.len()).unwrap());
                    legal[j as usize].action_index(state.n_tubes())
                }
            }
        };
        let out = episode_step(&state, action, moves, &rules).unwrap();
        steps.push(RolloutStep {
            action,
            state: puzzle_code::encode(&out.state),
            units_moved: out.units_moved,
            reward: out.reward,
            illegal: out.illegal,
            dead_end: out.dead_end,
            solved: out.solved,
            terminated: out.terminated,
            truncated: out.truncated,
            moves_so_far: out.moves_so_far,
        });
        state = out.state;
        moves = out.moves_so_far;
        if out.terminated || out.truncated {
            break;
        }
    }
    RolloutCase {
        name,
        puzzle_code: puzzle_code::encode(&start),
        opt_moves,
        move_limit_k: k,
        move_limit: rules.move_limit,
        shaping_gamma: rules.shaping_gamma,
        dead_end_max_states: rules.dead_end_max_states,
        steps,
    }
}

/// Environment rollouts (Phase 5): the Python `step`, `env_step`, `batch_env_step` and
/// `WaterSortEnv` must reproduce every state, reward and flag exactly.
/// Rollout starts: shuffled standard fills (as in [`sample_states`]) and distributed states.
fn rollout_starts() -> Vec<(String, State)> {
    let mut starts: Vec<(String, State)> = Vec::new();
    for params in [p(3, 3, 1), p(4, 4, 2), p(6, 4, 2)] {
        for seed in [0, 1, 42] {
            let state = State::from_fill(params, &shuffled_units(params, seed)).unwrap();
            starts.push((
                format!(
                    "standard_{}x{}_{}_{}",
                    params.n_colors,
                    params.capacity,
                    params.n_empty,
                    hex(seed)
                ),
                state,
            ));
        }
    }
    for params in [p(4, 4, 2), p(6, 4, 2)] {
        for seed in [0, 7] {
            let mut rng = ChaCha20Rng::seed_from_u64(seed);
            let heights = sample_heights(&mut rng, params);
            let mut units = State::sorted_units(params);
            fisher_yates(&mut rng, &mut units);
            let state = State::from_heights(params, &heights, &units).unwrap();
            starts.push((
                format!(
                    "distributed_{}x{}_{}_{}",
                    params.n_colors,
                    params.capacity,
                    params.n_empty,
                    hex(seed)
                ),
                state,
            ));
        }
    }
    starts
}

/// Hand-made dead ends: one pour into a position with no legal move, and an unsolvable start
/// that only the solver check ends.
fn dead_end_cases() -> Vec<RolloutCase> {
    let mut cases = Vec::new();
    let dead = State::from_tubes(p(3, 2, 1), &[&[0, 2][..], &[1, 2], &[0, 1], &[]]).unwrap();
    let opt = solve(&dead, &SolverLimits::default()).opt_moves();
    let no_check = EpisodeRules {
        move_limit: move_limit(4, opt.unwrap()),
        shaping_gamma: None,
        dead_end_max_states: None,
    };
    cases.push(rollout(
        "dead_end_no_legal_move".into(),
        dead,
        opt,
        4,
        no_check,
        Some(&[2 * 4 + 3]),
        0,
    ));
    let unsolvable =
        State::from_tubes(p(3, 3, 1), &[&[][..], &[0, 1, 1], &[0, 2, 2], &[0, 1, 2]]).unwrap();
    for check in [None, Some(10_000)] {
        let rules = EpisodeRules {
            move_limit: 6,
            shaping_gamma: None,
            dead_end_max_states: check,
        };
        let name = format!("unsolvable_dead_end_check_{}", check.is_some());
        cases.push(rollout(name, unsolvable, None, 4, rules, None, 99));
    }
    cases
}

#[test]
fn rollouts() {
    let mut cases = Vec::new();
    for (i, (name, start)) in rollout_starts().into_iter().enumerate() {
        let SolveResult::Solvable {
            opt_moves,
            solution,
            ..
        } = solve(&start, &SolverLimits::default())
        else {
            continue;
        };
        if opt_moves == 0 {
            continue;
        }
        let rules = |k: u32, gamma: Option<f64>| EpisodeRules {
            move_limit: move_limit(k, opt_moves),
            shaping_gamma: gamma,
            dead_end_max_states: None,
        };
        let optimal: Vec<usize> = solution
            .iter()
            .map(|m| m.action_index(start.n_tubes()))
            .collect();
        cases.push(rollout(
            format!("{name}_optimal"),
            start,
            Some(opt_moves),
            4,
            rules(4, None),
            Some(&optimal),
            0,
        ));
        cases.push(rollout(
            format!("{name}_optimal_shaped"),
            start,
            Some(opt_moves),
            4,
            rules(4, Some(0.99)),
            Some(&optimal),
            0,
        ));
        for (j, (k, gamma)) in [(1, None), (2, Some(0.9)), (4, None)]
            .into_iter()
            .enumerate()
        {
            let seed = splitmix64((i * 16 + j) as u64);
            cases.push(rollout(
                format!("{name}_mixed_k{k}"),
                start,
                Some(opt_moves),
                k,
                rules(k, gamma),
                None,
                seed,
            ));
        }
    }
    cases.extend(dead_end_cases());
    assert!(cases.iter().any(|c| c.steps.iter().any(|s| s.truncated)));
    assert!(cases.iter().any(|c| c.steps.iter().any(|s| s.illegal)));
    assert!(cases.iter().any(|c| c.steps.iter().any(|s| s.solved)));
    assert!(
        cases
            .iter()
            .filter(|c| c.steps.iter().any(|s| s.dead_end))
            .count()
            >= 2
    );
    check(
        "rollouts.json",
        &Rollouts {
            rules: "water_sort_core::episode::episode_step; actions are from * n_tubes + to".into(),
            cases,
        },
    );
}
