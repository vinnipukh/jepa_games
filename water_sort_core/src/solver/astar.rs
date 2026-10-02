//! A* search with the `segments - n_colors` heuristic (D9).

use core::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{Arena, Insert, Key, KeySearch, SolveResult, SolverLimits, heuristic, with_key};
use crate::moves::{apply_unchecked, solver_moves};
use crate::state::State;

/// Solves `s` optimally with A*.
///
/// The heuristic is consistent, so the first time a solved state is popped its depth is optimal.
/// The open list is ordered by `(f, -g, insertion counter)`; the counter makes tie-breaking, and
/// with it the returned solution, deterministic.
pub fn solve_astar(s: &State, limits: &SolverLimits) -> SolveResult {
    with_key(s.params(), AStar { root: *s, limits })
}

struct AStar<'a> {
    root: State,
    limits: &'a SolverLimits,
}

/// Open-list entry: `(f, u32::MAX - g, counter, node)`, popped smallest first.
type Entry = Reverse<(u32, u32, u64, u32)>;

impl KeySearch for AStar<'_> {
    type Output = SolveResult;

    fn run<K: Key>(self, bits: u32) -> SolveResult {
        let mut arena = Arena::<K>::new(&self.root, bits, self.limits);
        let mut closed = vec![false];
        let mut open: BinaryHeap<Entry> = BinaryHeap::new();
        let mut counter = 0u64;
        open.push(Reverse((heuristic(&self.root), u32::MAX, counter, 0)));
        let mut moves = Vec::new();
        while let Some(Reverse((_, inv_g, _, idx))) = open.pop() {
            let g = u32::MAX - inv_g;
            if closed[idx as usize] || g != arena.nodes[idx as usize].g {
                continue; // stale entry
            }
            let state = arena.state(idx);
            if state.is_solved() {
                return arena.solution(idx, &self.root);
            }
            closed[idx as usize] = true;
            if arena.tick() {
                return arena.timeout();
            }
            solver_moves(&state, &mut moves);
            for &m in &moves {
                let (child, _) = apply_unchecked(&state, m);
                let child_g = g + 1;
                let child_idx = match arena.insert(arena.key_of(&child), idx, child_g) {
                    Insert::New(i) => {
                        closed.push(false);
                        i
                    }
                    Insert::Existing(i) => {
                        let node = &mut arena.nodes[i as usize];
                        if closed[i as usize] || node.g <= child_g {
                            continue;
                        }
                        node.g = child_g;
                        node.parent = idx;
                        i
                    }
                    Insert::Full => return arena.timeout(),
                };
                counter += 1;
                let f = child_g + heuristic(&child);
                open.push(Reverse((f, u32::MAX - child_g, counter, child_idx)));
            }
        }
        arena.unsolvable()
    }
}
