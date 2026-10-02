//! Every generated puzzle's solution replays to a solved state in exactly `opt_moves` moves, the
//! record is consistent with its state and layout, and scrambles are never unsolvable.

use turan_water_sort::{Turan, TuranStrategy, no_adjacent_same_color};
use water_sort_core::{
    GenConfig, Generator, Layout, Params, RejectionCounts, canonical_hash, puzzle_code, replay,
    splitmix64,
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

fn check(generator: Turan, count: u64) {
    let cfg = GenConfig::default();
    for i in 0..count {
        let seed = splitmix64(0x7E91_A500 ^ i);
        let params = PARAMS[usize::try_from(i).unwrap() % PARAMS.len()];
        let (g, counts): (_, RejectionCounts) =
            generator.generate_traced(&params, seed, &cfg).unwrap();
        let ctx = format!("{generator:?} {params:?} seed {seed:016x}");
        assert_eq!(g.solution.len(), g.opt_moves as usize, "{ctx}");
        let end = replay(&g.state, &g.solution).unwrap_or_else(|| panic!("{ctx}: illegal move"));
        assert!(end.is_solved(), "{ctx}: replay does not solve");
        assert!(!g.state.is_solved() && g.opt_moves >= cfg.min_opt, "{ctx}");
        assert!(g.state.layout_matches(generator.layout), "{ctx}");
        assert_eq!(g.canonical_hash, canonical_hash(&g.state), "{ctx}");
        assert_eq!(puzzle_code::decode(&g.puzzle_code), Ok(g.state), "{ctx}");
        match generator.strategy {
            TuranStrategy::Scramble { .. } | TuranStrategy::ReverseSearch { .. } => {
                assert_eq!(counts.unsolvable, 0, "{ctx}");
            }
            TuranStrategy::PourWalk { .. } => {}
            TuranStrategy::Constrained => assert!(no_adjacent_same_color(&g.state), "{ctx}"),
        }
    }
}

fn check_all(count: u64) {
    for layout in Layout::ALL {
        check(Turan::new(TuranStrategy::default(), layout), count);
        check(Turan::new(TuranStrategy::Constrained, layout), count);
        check(
            Turan::new(TuranStrategy::DEFAULT_REVERSE_SEARCH, layout),
            count,
        );
    }
    check(
        Turan::new(TuranStrategy::DEFAULT_POUR_WALK, Layout::Distributed),
        count,
    );
}

#[test]
fn replays_solve() {
    check_all(150);
}

#[test]
#[ignore = "release-mode heavy test: 10k generations per (strategy, layout)"]
fn replays_solve_10k() {
    check_all(10_000);
}
