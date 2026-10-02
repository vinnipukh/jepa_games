//! Every generated puzzle's solution replays to a solved state in exactly `opt_moves` moves, and
//! the record is consistent with its state and layout.

use uniform_water_sort::Uniform;
use water_sort_core::{
    GenConfig, Generator, Layout, Params, canonical_hash, puzzle_code, replay, splitmix64,
};

const fn p(n_colors: u8, capacity: u8, n_empty: u8) -> Params {
    Params {
        n_colors,
        capacity,
        n_empty,
    }
}

const PARAMS: [Params; 6] = [
    p(3, 3, 1),
    p(4, 3, 2),
    p(4, 4, 2),
    p(5, 4, 2),
    p(6, 4, 2),
    p(5, 3, 1),
];

/// Generates `count` puzzles per layout from pseudo-random fixed seeds, cycling through
/// [`PARAMS`].
fn check_replays(count: u64) {
    for layout in Layout::ALL {
        check_layout(Uniform::new(layout), count);
    }
}

fn check_layout(generator: Uniform, count: u64) {
    let cfg = GenConfig::default();
    for i in 0..count {
        let seed = splitmix64(0x00E7_1A50 ^ i);
        let params = PARAMS[usize::try_from(i).unwrap() % PARAMS.len()];
        let g = generator.generate(&params, seed, &cfg).unwrap();
        assert!(g.state.layout_matches(generator.layout), "seed {seed:016x}");
        assert_eq!(g.solution.len(), g.opt_moves as usize, "seed {seed:016x}");
        let end = replay(&g.state, &g.solution)
            .unwrap_or_else(|| panic!("seed {seed:016x}: illegal move in the solution"));
        assert!(end.is_solved(), "seed {seed:016x}: replay does not solve");
        assert!(!g.state.is_solved() && g.opt_moves >= cfg.min_opt);
        assert_eq!(g.canonical_hash, canonical_hash(&g.state));
        assert_eq!(puzzle_code::decode(&g.puzzle_code), Ok(g.state));
    }
}

#[test]
fn replays_solve() {
    check_replays(300);
}

#[test]
#[ignore = "release-mode heavy test: 10k generations per layout"]
fn replays_solve_10k() {
    check_replays(10_000);
}
