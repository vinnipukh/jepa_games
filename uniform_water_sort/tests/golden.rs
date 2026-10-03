//! Golden vectors: 20 fixed seeds x 3 configs x 2 layouts -> committed puzzles in
//! `tests/golden/*.json`. The standard-layout files are the Phase 2 ones, unchanged.
//!
//! CI checks these on Linux and Windows, so the same `(params, seed, GenConfig, layout)` gives the
//! same puzzle everywhere. Regenerate with `WATER_SORT_BLESS=1 cargo test -p uniform_water_sort --test
//! golden` and review the diff: a change means old seeds no longer reproduce their puzzles, which
//! needs a `Uniform::VERSION` bump.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uniform_water_sort::Uniform;
use water_sort_core::{
    GenConfig, Generator, Layout, MetricsConfig, Move, Params, canonical_hash, puzzle_code, replay,
    splitmix64,
};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

/// 64-bit values as 16-digit hex strings: JSON numbers above 2^53 are not exact in JavaScript.
fn hex(x: u64) -> String {
    format!("{x:016x}")
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Case {
    seed: String,
    attempts: u32,
    opt_moves: u32,
    puzzle_code: String,
    canonical_hash: String,
    solution: Vec<Move>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Golden {
    generator: String,
    version: u32,
    variant: String,
    params: Params,
    config: GenConfig,
    cases: Vec<Case>,
}

const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
    Params {
        n_colors,
        capacity,
        n_empty,
    }
}

/// Edge-case seeds, then seeds derived with `splitmix64`.
fn seeds() -> Vec<u64> {
    let mut seeds = vec![0, 1, 42, 0xDEAD_BEEF, 1 << 63, u64::MAX];
    seeds.extend((0..14).map(|i| splitmix64(0x5EED_0000 + i)));
    seeds
}

fn configs() -> [(&'static str, Params, GenConfig); 3] {
    let base = GenConfig::default();
    [
        ("3x3_1", p(3, 3, 1), base),
        ("4x4_2", p(4, 4, 2), GenConfig { min_opt: 5, ..base }),
        (
            "6x4_2",
            p(6, 4, 2),
            GenConfig {
                min_opt: 10,
                max_opt: None,
                max_attempts: 1000,
                max_states: 1_000_000,
                metrics: MetricsConfig {
                    random_rollouts: 16,
                    ..MetricsConfig::default()
                },
            },
        ),
    ]
}

/// The Phase 2 names for the standard layout, `uniform_distributed_*` for the other.
fn file_name(layout: Layout, config: &str) -> String {
    match layout {
        Layout::Standard => format!("uniform_{config}.json"),
        Layout::Distributed => format!("uniform_distributed_{config}.json"),
    }
}

fn build(generator: Uniform, params: Params, config: GenConfig) -> Golden {
    let cases = seeds()
        .into_iter()
        .map(|seed| {
            let g = generator.generate(&params, seed, &config).unwrap();
            assert_eq!(g, generator.generate(&params, seed, &config).unwrap());
            assert!(g.state.layout_matches(generator.layout));
            assert!(replay(&g.state, &g.solution).unwrap().is_solved());
            assert_eq!(puzzle_code::decode(&g.puzzle_code), Ok(g.state));
            assert_eq!(g.canonical_hash, canonical_hash(&g.state));
            Case {
                seed: hex(seed),
                attempts: g.attempts,
                opt_moves: g.opt_moves,
                puzzle_code: g.puzzle_code,
                canonical_hash: hex(g.canonical_hash),
                solution: g.solution,
            }
        })
        .collect();
    Golden {
        generator: Uniform::ID.into(),
        version: Uniform::VERSION,
        variant: generator.variant(),
        params,
        config,
        cases,
    }
}

#[test]
fn golden_puzzles() {
    let bless = std::env::var_os("WATER_SORT_BLESS").is_some();
    for layout in Layout::ALL {
        for (config_name, params, config) in configs() {
            let name = file_name(layout, config_name);
            let actual = build(Uniform::new(layout), params, config);
            check(&name, &actual, bless);
        }
    }
}

fn check(name: &str, actual: &Golden, bless: bool) {
    let path = golden_path(name);
    if bless {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut json = serde_json::to_string_pretty(actual).unwrap();
        json.push('\n');
        std::fs::write(&path, json).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with WATER_SORT_BLESS=1)", path.display()));
    let expected: Golden = serde_json::from_str(&text).unwrap();
    assert_eq!(
        &expected, actual,
        "{name} differs from the committed golden file"
    );
}
