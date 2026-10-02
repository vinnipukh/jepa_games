//! Breadth-first search: the optimal reference solver for small puzzles and tests.

use std::collections::VecDeque;

use super::{Arena, Insert, Key, KeySearch, SolveResult, SolverLimits, with_key};
use crate::moves::{apply_unchecked, solver_moves};
use crate::state::State;

/// Solves `s` optimally by breadth-first search over [`crate::solver_key`] states.
pub fn solve_bfs(s: &State, limits: &SolverLimits) -> SolveResult {
    with_key(s.params(), Bfs { root: *s, limits })
}

struct Bfs<'a> {
    root: State,
    limits: &'a SolverLimits,
}

impl KeySearch for Bfs<'_> {
    type Output = SolveResult;

    fn run<K: Key>(self, bits: u32) -> SolveResult {
        let mut arena = Arena::<K>::new(&self.root, bits, self.limits);
        if self.root.is_solved() {
            return arena.solution(0, &self.root);
        }
        let mut queue = VecDeque::from([0u32]);
        let mut moves = Vec::new();
        while let Some(idx) = queue.pop_front() {
            if arena.tick() {
                return arena.timeout();
            }
            let state = arena.state(idx);
            let g = arena.nodes[idx as usize].g;
            solver_moves(&state, &mut moves);
            for &m in &moves {
                let (child, _) = apply_unchecked(&state, m);
                match arena.insert(arena.key_of(&child), idx, g + 1) {
                    Insert::New(child_idx) => {
                        // In BFS the first time a solved state is generated it is at minimal
                        // depth.
                        if child.is_solved() {
                            return arena.solution(child_idx, &self.root);
                        }
                        queue.push_back(child_idx);
                    }
                    Insert::Existing => {}
                    Insert::Full => return arena.timeout(),
                }
            }
        }
        arena.unsolvable()
    }
}
