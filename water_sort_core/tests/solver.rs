//! Solver tests: hand-made puzzles, replay of solutions, and BFS as the optimal reference.

mod common;

use common::{params, params_in, random_fill, random_state};
use proptest::prelude::*;
use water_sort_core::solver::solve_bfs;
use water_sort_core::{Move, SolveResult, SolverLimits, State, replay};

fn st(p: water_sort_core::Params, tubes: &[&[u8]]) -> State {
    State::from_tubes(p, tubes).unwrap()
}

type Solver = fn(&State, &SolverLimits) -> SolveResult;

fn solvers() -> Vec<(&'static str, Solver)> {
    vec![("bfs", solve_bfs)]
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
