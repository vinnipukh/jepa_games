//! Benchmarks for the hot paths: `apply`, `canonical_full` and solving reference puzzles.
//!
//! Run with `cargo bench -p water_sort_core`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use water_sort_core::{
    Params, SolverLimits, State, apply, canonical_full, canonical_hash, fisher_yates, legal_moves,
    solve, solver_key,
};

fn fill(params: Params, seed: u64) -> State {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut units = State::sorted_units(params);
    fisher_yates(&mut rng, &mut units);
    State::from_fill(params, &units).unwrap()
}

/// The first `count` solvable fills for `params`, from seeds 0, 1, 2, …
fn solvable(params: Params, count: usize) -> Vec<State> {
    (0..)
        .map(|seed| fill(params, seed))
        .filter(|s| solve(s, &SolverLimits::default()).opt_moves().is_some())
        .take(count)
        .collect()
}

fn bench_moves(c: &mut Criterion) {
    let s = fill(Params::with_colors(12), 1);
    let moves: Vec<_> = legal_moves(&s).collect();
    c.bench_function("apply/all_legal_12c", |b| {
        b.iter(|| {
            for &m in &moves {
                black_box(apply(black_box(&s), m).unwrap());
            }
        });
    });
    c.bench_function("legal_moves/12c", |b| {
        b.iter(|| legal_moves(black_box(&s)).count());
    });
}

fn bench_canon(c: &mut Criterion) {
    for n in [6u8, 12] {
        let s = fill(Params::with_colors(n), 3);
        c.bench_function(&format!("canonical_full/{n}c"), |b| {
            b.iter(|| canonical_full(black_box(&s)));
        });
        c.bench_function(&format!("canonical_hash/{n}c"), |b| {
            b.iter(|| canonical_hash(black_box(&s)));
        });
        c.bench_function(&format!("solver_key/{n}c"), |b| {
            b.iter(|| solver_key(black_box(&s)));
        });
    }
    // Worst case for the canonical search: 7 copies of a 2-color pattern (large automorphism
    // group).
    let p = Params::with_colors(14);
    let tubes: Vec<Vec<u8>> = (0..14u8)
        .map(|i| {
            let j = i ^ 1;
            vec![i, j, i, j]
        })
        .chain([vec![], vec![]])
        .collect();
    let s = State::from_tubes(p, &tubes).unwrap();
    c.bench_function("canonical_full/14c_symmetric", |b| {
        b.iter(|| canonical_full(black_box(&s)));
    });
}

fn bench_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("solve");
    group.sample_size(10);
    for n in [4u8, 6, 8] {
        let puzzles = solvable(Params::with_colors(n), 5);
        group.bench_function(format!("astar/{n}c_cap4_e2"), |b| {
            b.iter(|| {
                for s in &puzzles {
                    black_box(solve(s, &SolverLimits::default()));
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_moves, bench_canon, bench_solve);
criterion_main!(benches);
