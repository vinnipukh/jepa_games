//! The golden vectors Rust and Python check, replayed on wasm32 (wasm-bindgen-test, Node), so the
//! same seed or puzzle code gives the same puzzle on every target (rule 2).
//!
//! Run with `cargo test --release -p water_sort_web --target wasm32-unknown-unknown` (the
//! runner, `wasm-bindgen-test-runner`, is set in `.cargo/config.toml`). The files are compiled
//! in with `include_str!`, since wasm32-unknown-unknown has no file system;
//! `tests/golden_files.rs` checks natively that the list below names every generator file.

#![cfg(target_arch = "wasm32")]

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::Deserialize;
use serde_json::Value;
use wasm_bindgen_test::wasm_bindgen_test;
use water_sort_core::{
    EpisodeRules, Move, Params as CoreParams, SolveResult, SolverLimits, State, bounded_u32,
    canonical_full, episode_step, fisher_yates, puzzle_code, solve, time_seed,
};
use water_sort_web::{
    JsParams as Params, Puzzle, Session, canonical_hash, generate_with_config, splitmix64, variant,
};

const CANONICAL: &str = include_str!("../../water_sort_core/tests/golden/canonical.json");
const PUZZLE_CODES: &str = include_str!("../../water_sort_core/tests/golden/puzzle_codes.json");
const ROLLOUTS: &str = include_str!("../../water_sort_core/tests/golden/rollouts.json");
const SAMPLING: &str = include_str!("../../water_sort_core/tests/golden/sampling.json");
const SEEDS: &str = include_str!("../../water_sort_core/tests/golden/seeds.json");
const SOLUTIONS: &str = include_str!("../../water_sort_core/tests/golden/solutions.json");

/// Every generator golden file (`tests/golden_files.rs` keeps this list complete).
pub const GENERATOR_FILES: &[(&str, &str)] = &[
    (
        "uniform_3x3_1.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_3x3_1.json"),
    ),
    (
        "uniform_4x4_2.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_4x4_2.json"),
    ),
    (
        "uniform_6x4_2.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_6x4_2.json"),
    ),
    (
        "uniform_distributed_3x3_1.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_distributed_3x3_1.json"),
    ),
    (
        "uniform_distributed_4x4_2.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_distributed_4x4_2.json"),
    ),
    (
        "uniform_distributed_6x4_2.json",
        include_str!("../../uniform_water_sort/tests/golden/uniform_distributed_6x4_2.json"),
    ),
    (
        "turan_constrained_distributed_3x3_1.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_constrained_distributed_3x3_1.json"
        ),
    ),
    (
        "turan_constrained_distributed_4x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_constrained_distributed_4x4_2.json"
        ),
    ),
    (
        "turan_constrained_distributed_6x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_constrained_distributed_6x4_2.json"
        ),
    ),
    (
        "turan_constrained_standard_3x3_1.json",
        include_str!("../../turan_water_sort/tests/golden/turan_constrained_standard_3x3_1.json"),
    ),
    (
        "turan_constrained_standard_4x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_constrained_standard_4x4_2.json"),
    ),
    (
        "turan_constrained_standard_6x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_constrained_standard_6x4_2.json"),
    ),
    (
        "turan_pour_walk_distributed_3x3_1.json",
        include_str!("../../turan_water_sort/tests/golden/turan_pour_walk_distributed_3x3_1.json"),
    ),
    (
        "turan_pour_walk_distributed_4x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_pour_walk_distributed_4x4_2.json"),
    ),
    (
        "turan_pour_walk_distributed_6x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_pour_walk_distributed_6x4_2.json"),
    ),
    (
        "turan_reverse_search_distributed_3x3_1.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_distributed_3x3_1.json"
        ),
    ),
    (
        "turan_reverse_search_distributed_4x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_distributed_4x4_2.json"
        ),
    ),
    (
        "turan_reverse_search_distributed_6x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_distributed_6x4_2.json"
        ),
    ),
    (
        "turan_reverse_search_standard_3x3_1.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_standard_3x3_1.json"
        ),
    ),
    (
        "turan_reverse_search_standard_4x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_standard_4x4_2.json"
        ),
    ),
    (
        "turan_reverse_search_standard_6x4_2.json",
        include_str!(
            "../../turan_water_sort/tests/golden/turan_reverse_search_standard_6x4_2.json"
        ),
    ),
    (
        "turan_scramble_distributed_3x3_1.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_distributed_3x3_1.json"),
    ),
    (
        "turan_scramble_distributed_4x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_distributed_4x4_2.json"),
    ),
    (
        "turan_scramble_distributed_6x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_distributed_6x4_2.json"),
    ),
    (
        "turan_scramble_standard_3x3_1.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_standard_3x3_1.json"),
    ),
    (
        "turan_scramble_standard_4x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_standard_4x4_2.json"),
    ),
    (
        "turan_scramble_standard_6x4_2.json",
        include_str!("../../turan_water_sort/tests/golden/turan_scramble_standard_6x4_2.json"),
    ),
];

fn hex_u64(v: &Value) -> u64 {
    u64::from_str_radix(v.as_str().expect("hex string"), 16).expect("hex")
}

#[derive(Deserialize)]
struct CanonicalCase {
    state: State,
    canonical: State,
    hash: String,
}

#[wasm_bindgen_test]
fn canonical_vectors() {
    let cases: Vec<CanonicalCase> = serde_json::from_str(CANONICAL).unwrap();
    assert!(cases.len() > 30);
    for c in cases {
        assert_eq!(canonical_full(&c.state), c.canonical);
        let code = puzzle_code::encode(&c.state);
        assert_eq!(canonical_hash(&code).unwrap(), c.hash);
    }
}

#[derive(Deserialize)]
struct CodeCase {
    state: State,
    code: String,
}

#[wasm_bindgen_test]
fn puzzle_code_vectors() {
    let cases: Vec<CodeCase> = serde_json::from_str(PUZZLE_CODES).unwrap();
    for c in cases {
        assert_eq!(puzzle_code::encode(&c.state), c.code);
        assert_eq!(puzzle_code::decode(&c.code), Ok(c.state));
    }
}

#[derive(Deserialize)]
struct SolutionCase {
    state: State,
    max_states: u64,
    result: SolveResult,
}

#[wasm_bindgen_test]
fn solution_vectors() {
    let cases: Vec<SolutionCase> = serde_json::from_str(SOLUTIONS).unwrap();
    for c in cases {
        // Exact, including the expanded-state count.
        assert_eq!(
            solve(&c.state, &SolverLimits::states(c.max_states)),
            c.result
        );
    }
}

#[wasm_bindgen_test]
fn seed_vectors() {
    let d: Value = serde_json::from_str(SEEDS).unwrap();
    for c in d["splitmix64"].as_array().unwrap() {
        let input = c["input"].as_str().unwrap();
        assert_eq!(splitmix64(input).unwrap(), c["output"].as_str().unwrap());
    }
    for c in d["time_seed"].as_array().unwrap() {
        let seed = time_seed(hex_u64(&c["now_nanos"]), c["counter"].as_u64().unwrap());
        assert_eq!(seed, hex_u64(&c["seed"]));
    }
}

#[wasm_bindgen_test]
fn sampling_vectors() {
    let d: Value = serde_json::from_str(SAMPLING).unwrap();
    for c in d["bounded_u32"].as_array().unwrap() {
        let mut rng = ChaCha20Rng::seed_from_u64(hex_u64(&c["seed"]));
        let n = u32::try_from(c["n"].as_u64().unwrap()).unwrap();
        let values: Vec<u64> = (0..8)
            .map(|_| u64::from(bounded_u32(&mut rng, n)))
            .collect();
        let expected: Vec<u64> = c["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(values, expected);
    }
    for c in d["fisher_yates"].as_array().unwrap() {
        let params: CoreParams = serde_json::from_value(c["params"].clone()).unwrap();
        let mut rng = ChaCha20Rng::seed_from_u64(hex_u64(&c["seed"]));
        let mut units = State::sorted_units(params);
        fisher_yates(&mut rng, &mut units);
        let expected: Vec<u8> = serde_json::from_value(c["units"].clone()).unwrap();
        assert_eq!(units, expected);
    }
}

/// Every generator file through the web API: same attempts, optimum, code, hash and solution.
#[wasm_bindgen_test]
fn generator_vectors() {
    assert_eq!(GENERATOR_FILES.len(), 27);
    let mut puzzles = 0;
    for (name, text) in GENERATOR_FILES {
        let d: Value = serde_json::from_str(text).unwrap();
        let generator = d["generator"].as_str().unwrap();
        let v = d["variant"].as_str().unwrap();
        let layout = if v.contains("layout=distributed") {
            "distributed"
        } else {
            "standard"
        };
        let strategy = (generator == "turan").then(|| v.to_owned());
        assert_eq!(
            variant(generator, strategy.clone(), Some(layout.into())).unwrap(),
            v,
            "{name}"
        );
        let p = &d["params"];
        let byte = |k: &str| u8::try_from(p[k].as_u64().unwrap()).unwrap();
        let params = Params::new(byte("n_colors"), byte("capacity"), byte("n_empty"));
        let config = d["config"].to_string();
        for c in d["cases"].as_array().unwrap() {
            let seed = c["seed"].as_str().unwrap();
            let puzzle = generate_with_config(
                generator,
                &params,
                Some(seed.into()),
                strategy.clone(),
                Some(layout.into()),
                &config,
            )
            .unwrap_or_else(|_| panic!("{name} seed {seed} failed"));
            assert_eq!(puzzle.seed().as_deref(), Some(seed));
            assert_eq!(
                u64::from(puzzle.generator_version().unwrap()),
                d["version"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(puzzle.attempts().unwrap()),
                c["attempts"].as_u64().unwrap(),
                "{name} {seed}"
            );
            assert_eq!(
                u64::from(puzzle.opt_moves().unwrap()),
                c["opt_moves"].as_u64().unwrap()
            );
            assert_eq!(
                puzzle.puzzle_code(),
                c["puzzle_code"].as_str().unwrap(),
                "{name} {seed}"
            );
            assert_eq!(
                puzzle.canonical_hash(),
                c["canonical_hash"].as_str().unwrap()
            );
            let solution: Vec<Move> = serde_json::from_value(c["solution"].clone()).unwrap();
            assert_eq!(puzzle.solution_moves().unwrap(), solution.as_slice());
            puzzles += 1;
        }
    }
    assert_eq!(puzzles, 27 * 20);
}

// The flags mirror the Gymnasium step outputs.
#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
struct RolloutStep {
    action: usize,
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

#[derive(Deserialize)]
struct RolloutCase {
    name: String,
    puzzle_code: String,
    opt_moves: Option<u32>,
    move_limit: u32,
    shaping_gamma: Option<f64>,
    dead_end_max_states: Option<u64>,
    steps: Vec<RolloutStep>,
}

#[derive(Deserialize)]
struct Rollouts {
    cases: Vec<RolloutCase>,
}

fn rollouts() -> Vec<RolloutCase> {
    serde_json::from_str::<Rollouts>(ROLLOUTS).unwrap().cases
}

/// The episode rules on wasm32: every state, reward (exact) and flag.
#[wasm_bindgen_test]
fn rollouts_through_episode_step() {
    let cases = rollouts();
    assert!(cases.len() > 50);
    for case in cases {
        let rules = EpisodeRules {
            move_limit: case.move_limit,
            shaping_gamma: case.shaping_gamma,
            dead_end_max_states: case.dead_end_max_states,
        };
        let mut state = puzzle_code::decode(&case.puzzle_code).unwrap();
        let mut moves = 0;
        for st in &case.steps {
            let out = episode_step(&state, st.action, moves, &rules).unwrap();
            assert_eq!(puzzle_code::encode(&out.state), st.state, "{}", case.name);
            assert_eq!(out.reward.to_bits(), st.reward.to_bits(), "{}", case.name);
            assert_eq!(
                (
                    out.units_moved,
                    out.illegal,
                    out.dead_end,
                    out.solved,
                    out.terminated,
                    out.truncated,
                    out.moves_so_far
                ),
                (
                    st.units_moved,
                    st.illegal,
                    st.dead_end,
                    st.solved,
                    st.terminated,
                    st.truncated,
                    st.moves_so_far
                ),
            );
            state = out.state;
            moves = out.moves_so_far;
        }
    }
}

/// The same rollouts through the web `Session` the player uses: legal pours move the same
/// units to the same state and count; illegal ones change nothing and do not count.
#[wasm_bindgen_test]
fn rollouts_through_session() {
    let mut replayed = 0;
    for case in rollouts() {
        if case.opt_moves.is_none() {
            continue; // unsolvable start
        }
        let puzzle = water_sort_web::from_code(&case.puzzle_code, None).unwrap();
        assert_eq!(puzzle.opt_moves(), case.opt_moves, "{}", case.name);
        let n = puzzle.n_tubes();
        let mut session = Session::new(&puzzle);
        let mut counted = 0;
        for st in &case.steps {
            let from = u8::try_from(st.action / n).unwrap();
            let to = u8::try_from(st.action % n).unwrap();
            let legal = session
                .legal_moves()
                .contains(&u16::try_from(st.action).unwrap());
            assert_eq!(legal, !st.illegal, "{}", case.name);
            assert_eq!(session.why_illegal(from, to).is_some(), st.illegal);
            let units = session.pour(from, to);
            assert_eq!(units, st.units_moved, "{}", case.name);
            counted += u32::from(!st.illegal);
            assert_eq!(session.moves_counted(), counted);
            assert_eq!(session.state_code(), st.state, "{}", case.name);
            assert_eq!(session.is_solved(), st.solved);
        }
        replayed += 1;
    }
    assert!(replayed > 50);
}

/// A puzzle survives the worker -> page JSON round trip unchanged.
#[wasm_bindgen_test]
fn puzzle_json_round_trip() {
    let (_, text) = GENERATOR_FILES[0];
    let d: Value = serde_json::from_str(text).unwrap();
    let code = d["cases"][0]["puzzle_code"].as_str().unwrap();
    let p = water_sort_web::from_code(code, None).unwrap();
    let back = Puzzle::from_json(&p.to_json().unwrap()).unwrap();
    assert_eq!(back.puzzle_code(), p.puzzle_code());
    assert_eq!(back.solution(), p.solution());
}
