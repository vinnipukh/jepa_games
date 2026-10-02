//! Solver tests: hand-made puzzles, replay of solutions, and BFS as the optimal reference.

mod common;

use common::{params, params_in, random_fill, random_state};
use proptest::prelude::*;
use water_sort_core::solver::{solve_astar, solve_bfs};
use water_sort_core::{
    Move, SolveResult, SolverLimits, State, heuristic, replay, reverse_moves, unapply,
};

fn st(p: water_sort_core::Params, tubes: &[&[u8]]) -> State {
    State::from_tubes(p, tubes).unwrap()
}

type Solver = fn(&State, &SolverLimits) -> SolveResult;

fn solvers() -> Vec<(&'static str, Solver)> {
    vec![("bfs", solve_bfs), ("astar", solve_astar)]
}

fn check_solution(s: &State, r: &SolveResult) {
    if let SolveResult::Solvable {
        opt_moves,
        solution,
        ..
    } = r
    {
        assert_eq!(solution.len(), *opt_moves as usize);
        assert!(
            replay(s, solution).unwrap().is_solved(),
            "{s}: {solution:?}"
        );
    }
}

#[test]
fn already_solved() {
    for (name, solve) in solvers() {
        let s = State::solved(params(3, 4, 2)).unwrap();
        let r = solve(&s, &SolverLimits::default());
        assert_eq!(r.opt_moves(), Some(0), "{name}");
        check_solution(&s, &r);
    }
}

#[test]
fn small_known_optimum() {
    let p = params(2, 2, 1);
    let s = st(p, &[&[0, 1], &[1, 0], &[]]);
    for (name, solve) in solvers() {
        let r = solve(&s, &SolverLimits::default());
        assert_eq!(r.opt_moves(), Some(3), "{name}");
        check_solution(&s, &r);
    }
    let s = st(params(3, 2, 1), &[&[0, 1], &[1, 2], &[2, 0], &[]]);
    for (name, solve) in solvers() {
        let r = solve(&s, &SolverLimits::default());
        assert_eq!(r.opt_moves(), Some(4), "{name}");
        check_solution(&s, &r);
    }
    // One pour away.
    let s = st(params(3, 3, 1), &[&[0, 0], &[1, 1, 1], &[2, 2, 2], &[0]]);
    for (name, solve) in solvers() {
        let r = solve(&s, &SolverLimits::default());
        assert_eq!(r.opt_moves(), Some(1), "{name}");
        let SolveResult::Solvable { solution, .. } = &r else {
            unreachable!()
        };
        assert!(solution == &[Move::new(0, 3)] || solution == &[Move::new(3, 0)]);
    }
}

#[test]
fn known_unsolvable() {
    let cases = [
        // No empty tube and no matching tops: no legal move at all.
        st(params(2, 2, 0), &[&[0, 1], &[1, 0]]),
        st(params(2, 3, 0), &[&[0, 0, 1], &[1, 1, 0]]),
        // Legal moves exist, but no sequence sorts the bottom 0s.
        st(params(3, 3, 1), &[&[0, 2, 1], &[0, 1, 1], &[0, 2, 2], &[]]),
        st(
            params(4, 4, 1),
            &[
                &[1, 3, 2, 2],
                &[1, 0, 0, 0],
                &[3, 2, 0, 3],
                &[3, 1, 1, 2],
                &[],
            ],
        ),
    ];
    for s in &cases {
        for (name, solve) in solvers() {
            let r = solve(s, &SolverLimits::default());
            assert!(
                matches!(r, SolveResult::Unsolvable { .. }),
                "{name}: {s} -> {r:?}"
            );
        }
    }
}
#[test]
fn state_limit_gives_timeout() {
    let s = random_fill(params(6, 4, 2), 7);
    for (name, solve) in solvers() {
        let r = solve(&s, &SolverLimits::states(10));
        assert!(matches!(r, SolveResult::Timeout { .. }), "{name}: {r:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Applying the returned solution reaches a solved state in exactly `opt_moves` moves.
    #[test]
    fn solutions_replay(p in params_in(2..=4, 2..=4, 1..=2), seed in any::<u64>(), fill in any::<bool>()) {
        let s = if fill { random_fill(p, seed) } else { random_state(p, seed) };
        for (_, solve) in solvers() {
            let r = solve(&s, &SolverLimits::default());
            let timed_out = matches!(r, SolveResult::Timeout { .. });
            prop_assert!(!timed_out);
            check_solution(&s, &r);
        }
    }
}

fn same_outcome(a: &SolveResult, b: &SolveResult) -> bool {
    match (a, b) {
        (
            SolveResult::Solvable { opt_moves: x, .. },
            SolveResult::Solvable { opt_moves: y, .. },
        ) => x == y,
        (SolveResult::Unsolvable { .. }, SolveResult::Unsolvable { .. }) => true,
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn astar_matches_bfs(p in params_in(2..=4, 2..=4, 1..=2), seed in any::<u64>(), fill in any::<bool>()) {
        let s = if fill { random_fill(p, seed) } else { random_state(p, seed) };
        let limits = SolverLimits::default();
        let (bfs, astar) = (solve_bfs(&s, &limits), solve_astar(&s, &limits));
        prop_assert!(same_outcome(&bfs, &astar), "{}: {:?} vs {:?}", s, bfs, astar);
    }

    /// `h(s) <= true distance`, with the distance from BFS.
    #[test]
    fn heuristic_is_admissible(p in params_in(2..=4, 2..=4, 1..=2), seed in any::<u64>()) {
        let s = random_state(p, seed);
        if let Some(opt) = solve_bfs(&s, &SolverLimits::default()).opt_moves() {
            prop_assert!(heuristic(&s) <= opt, "{}: h = {} > {}", s, heuristic(&s), opt);
        }
    }

    /// A random walk of reverse moves from a solved state always yields a solvable state.
    #[test]
    fn reverse_walks_are_solvable(p in params_in(2..=5, 2..=4, 1..=2), seed in any::<u64>(), steps in 0usize..40) {
        let mut rng = common::rng(seed);
        let mut s = State::solved(p).unwrap();
        for _ in 0..steps {
            let moves = reverse_moves(&s);
            if moves.is_empty() {
                break;
            }
            s = unapply(&s, moves[common::below(&mut rng, moves.len())]).unwrap();
        }
        let r = solve_astar(&s, &SolverLimits::default());
        prop_assert!(r.opt_moves().is_some(), "{}: {:?}", s, r);
        check_solution(&s, &r);
    }
}

#[test]
fn solving_is_deterministic() {
    for seed in 0..20 {
        let s = random_fill(params(5, 4, 2), seed);
        let limits = SolverLimits::default();
        assert_eq!(solve_astar(&s, &limits), solve_astar(&s, &limits));
        assert_eq!(solve_bfs(&s, &limits), solve_bfs(&s, &limits));
    }
}

/// Phase 1 acceptance: A* returns the same `opt_moves` as BFS on 10,000 random small puzzles
/// (`n_colors` 2–4, capacity 3–4, `n_empty` 1–2). Run in release by the heavy-tests CI job.
#[test]
#[ignore = "heavy: run with --release -- --ignored"]
fn astar_equals_bfs_10k() {
    let configs: Vec<_> = (2..=4)
        .flat_map(|c| (3..=4).flat_map(move |k| (1..=2).map(move |e| params(c, k, e))))
        .collect();
    let limits = SolverLimits::default();
    let (mut solvable, mut unsolvable) = (0, 0);
    for i in 0..10_000u64 {
        let p = configs[usize::try_from(i).unwrap() % configs.len()];
        let s = random_fill(p, i);
        let (bfs, astar) = (solve_bfs(&s, &limits), solve_astar(&s, &limits));
        assert!(
            same_outcome(&bfs, &astar),
            "seed {i}: {s}: {bfs:?} vs {astar:?}"
        );
        check_solution(&s, &astar);
        if bfs.opt_moves().is_some() {
            solvable += 1;
        } else {
            unsolvable += 1;
        }
    }
    println!("solvable {solvable}, unsolvable {unsolvable}");
    assert!(solvable > 1000 && unsolvable > 100);
}

/// Plain BFS over raw labeled states with every game move: no keys, no pruning.
fn naive_bfs(s: &State) -> Option<u32> {
    let mut seen = std::collections::HashSet::from([*s]);
    let mut frontier = vec![*s];
    let mut depth = 0;
    while !frontier.is_empty() {
        if frontier.iter().any(State::is_solved) {
            return Some(depth);
        }
        let mut next = Vec::new();
        for cur in &frontier {
            for m in water_sort_core::legal_moves(cur) {
                let (child, _) = water_sort_core::apply(cur, m).unwrap();
                if seen.insert(child) {
                    next.push(child);
                }
            }
        }
        frontier = next;
        depth += 1;
    }
    None
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The solver's symmetry merging and move pruning never change the optimum.
    #[test]
    fn pruned_search_matches_naive_bfs(p in params_in(2..=3, 2..=3, 1..=2), seed in any::<u64>()) {
        let s = random_state(p, seed);
        prop_assert_eq!(solve_bfs(&s, &SolverLimits::default()).opt_moves(), naive_bfs(&s), "{}", s);
    }
}
