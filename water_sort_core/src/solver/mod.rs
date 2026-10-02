//! Optimal solvers: BFS (reference) and A* (default).
//!
//! Both search over [`solver_key`] states, which merge tube-order and (most) color-relabeling
//! symmetries. Keys are bit-packed into fixed-width arrays to keep memory low. The returned
//! solution is replayed on the caller's original labeled state, so its moves are real tube
//! indices.

mod astar;
mod bfs;

use core::hash::Hash;
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::canon::{canonical_full, solver_key};
use crate::moves::{Move, apply_unchecked, legal_moves};
use crate::params::{MAX_CAP, Params};
use crate::state::{EMPTY, State};

pub use astar::solve_astar;
pub use bfs::solve_bfs;

/// Search limits.
///
/// `max_states` bounds the number of distinct states stored, and with it memory. It is the only
/// limit generation may use (D11). `max_time` is for interactive use only (web, CLI `solve`):
/// it makes the result depend on machine speed. Reading the clock is not supported on
/// `wasm32-unknown-unknown`, so the web build must leave it `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SolverLimits {
    pub max_states: u64,
    pub max_time: Option<Duration>,
}

impl SolverLimits {
    pub const DEFAULT_MAX_STATES: u64 = 5_000_000;

    /// A state-count limit only.
    pub const fn states(max_states: u64) -> Self {
        Self {
            max_states,
            max_time: None,
        }
    }
}

impl Default for SolverLimits {
    fn default() -> Self {
        Self::states(Self::DEFAULT_MAX_STATES)
    }
}

/// Outcome of a search. `states_expanded` counts states taken off the frontier and expanded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolveResult {
    Solvable {
        opt_moves: u32,
        solution: Vec<Move>,
        states_expanded: u64,
    },
    /// The whole reachable state space was searched without finding a solved state.
    Unsolvable { states_expanded: u64 },
    /// A limit was hit before the search finished.
    Timeout { states_expanded: u64 },
}

impl SolveResult {
    pub const fn opt_moves(&self) -> Option<u32> {
        match self {
            Self::Solvable { opt_moves, .. } => Some(*opt_moves),
            _ => None,
        }
    }

    pub const fn states_expanded(&self) -> u64 {
        match self {
            Self::Solvable {
                states_expanded, ..
            }
            | Self::Unsolvable { states_expanded }
            | Self::Timeout { states_expanded } => *states_expanded,
        }
    }
}

/// Solves `s` optimally with A* ([`solve_astar`]).
pub fn solve(s: &State, limits: &SolverLimits) -> SolveResult {
    solve_astar(s, limits)
}

/// Admissible, consistent heuristic (D9): `segments - n_colors`.
///
/// A pour changes the number of maximal same-color runs by at most one, and a solved state has
/// exactly `n_colors` of them.
pub fn heuristic(s: &State) -> u32 {
    s.segments() - u32::from(s.params().n_colors)
}

/// A bit-packed [`solver_key`] state.
pub(crate) trait Key: Copy + Eq + Hash {
    fn pack(s: &State, bits: u32) -> Self;
    fn unpack(&self, params: Params, bits: u32) -> State;
}

impl<const W: usize> Key for [u64; W] {
    fn pack(s: &State, bits: u32) -> Self {
        let mut words = [0u64; W];
        let mut pos = 0u32;
        for i in 0..s.n_tubes() {
            for &c in s.padded_tube(i) {
                let v = if c == EMPTY { 0 } else { u64::from(c) + 1 };
                let (w, off) = ((pos / 64) as usize, pos % 64);
                words[w] |= v << off;
                if off + bits > 64 {
                    words[w + 1] |= v >> (64 - off);
                }
                pos += bits;
            }
        }
        words
    }

    fn unpack(&self, params: Params, bits: u32) -> State {
        let mask = (1u64 << bits) - 1;
        let mut tubes = vec![[EMPTY; MAX_CAP]; params.n_tubes()];
        let mut pos = 0u32;
        for tube in &mut tubes {
            for cell in tube.iter_mut().take(usize::from(params.capacity)) {
                let (w, off) = ((pos / 64) as usize, pos % 64);
                let mut v = self[w] >> off;
                if off + bits > 64 {
                    v |= self[w + 1] << (64 - off);
                }
                let v = v & mask;
                *cell = if v == 0 {
                    EMPTY
                } else {
                    u8::try_from(v - 1).expect("packed color fits in u8")
                };
                pos += bits;
            }
        }
        State::from_raw_tubes(params, &tubes)
    }
}

/// Bits per packed cell: enough for values `0..=n_colors` (0 = empty).
fn cell_bits(params: Params) -> u32 {
    u32::from(params.n_colors).bit_width()
}

/// Runs `search` with the narrowest key type that fits the params.
fn with_key<R>(params: Params, search: impl KeySearch<Output = R>) -> R {
    let bits = cell_bits(params);
    let total =
        u32::try_from(params.n_tubes() * usize::from(params.capacity)).unwrap_or(u32::MAX) * bits;
    match total.div_ceil(64) {
        0..=2 => search.run::<[u64; 2]>(bits),
        3..=4 => search.run::<[u64; 4]>(bits),
        5..=6 => search.run::<[u64; 6]>(bits),
        _ => search.run::<[u64; 10]>(bits),
    }
}

/// A search that is generic over the key width.
trait KeySearch {
    type Output;
    fn run<K: Key>(self, bits: u32) -> Self::Output;
}

/// One stored search node: its key, how it was reached, and its best known depth.
struct Node<K> {
    key: K,
    parent: u32,
    g: u32,
}

const NO_PARENT: u32 = u32::MAX;

/// The stored states of a search, with the state and time limits.
struct Arena<K> {
    nodes: Vec<Node<K>>,
    index: FxHashMap<K, u32>,
    params: Params,
    bits: u32,
    limits: SolverLimits,
    started: Option<Instant>,
    expanded: u64,
}

/// Result of inserting a successor into the arena.
enum Insert {
    New(u32),
    Existing(u32),
    Full,
}

impl<K: Key> Arena<K> {
    fn new(root: &State, bits: u32, limits: &SolverLimits) -> Self {
        let mut arena = Self {
            nodes: Vec::new(),
            index: FxHashMap::default(),
            params: root.params(),
            bits,
            limits: *limits,
            started: limits.max_time.map(|_| Instant::now()),
            expanded: 0,
        };
        let key = K::pack(&solver_key(root), bits);
        arena.nodes.push(Node {
            key,
            parent: NO_PARENT,
            g: 0,
        });
        arena.index.insert(key, 0);
        arena
    }

    fn key_of(&self, s: &State) -> K {
        K::pack(&solver_key(s), self.bits)
    }

    fn state(&self, idx: u32) -> State {
        self.nodes[idx as usize].key.unpack(self.params, self.bits)
    }

    fn insert(&mut self, key: K, parent: u32, g: u32) -> Insert {
        if let Some(&idx) = self.index.get(&key) {
            return Insert::Existing(idx);
        }
        if self.nodes.len() as u64 >= self.limits.max_states {
            return Insert::Full;
        }
        let idx = u32::try_from(self.nodes.len()).expect("node count fits in u32");
        self.nodes.push(Node { key, parent, g });
        self.index.insert(key, idx);
        Insert::New(idx)
    }

    /// Counts one expansion and reports whether the time limit has passed.
    fn tick(&mut self) -> bool {
        self.expanded += 1;
        match (self.started, self.limits.max_time) {
            (Some(start), Some(max)) if self.expanded.is_multiple_of(1024) => start.elapsed() > max,
            _ => false,
        }
    }

    fn timeout(&self) -> SolveResult {
        SolveResult::Timeout {
            states_expanded: self.expanded,
        }
    }

    fn unsolvable(&self) -> SolveResult {
        SolveResult::Unsolvable {
            states_expanded: self.expanded,
        }
    }

    /// Rebuilds the path to `goal` and replays it on the original labeled state.
    ///
    /// Each step picks the first legal move (in `(from, to)` order) whose result is symmetric to
    /// the next node on the path. Symmetry is tested with the exact [`canonical_full`], because
    /// [`solver_key`] can give symmetric states different keys.
    fn solution(&self, goal: u32, original: &State) -> SolveResult {
        let mut path = Vec::new();
        let mut idx = goal;
        while idx != NO_PARENT {
            path.push(idx);
            idx = self.nodes[idx as usize].parent;
        }
        path.reverse();
        let mut current = *original;
        let mut solution = Vec::with_capacity(path.len().saturating_sub(1));
        for &next in &path[1..] {
            let target = canonical_full(&self.state(next));
            let (m, s) = legal_moves(&current)
                .map(|m| (m, apply_unchecked(&current, m).0))
                .find(|(_, s)| canonical_full(s) == target)
                .expect("every search edge has a matching move in the labeled state");
            solution.push(m);
            current = s;
        }
        debug_assert!(current.is_solved());
        SolveResult::Solvable {
            opt_moves: u32::try_from(solution.len()).expect("solution length fits in u32"),
            solution,
            states_expanded: self.expanded,
        }
    }
}

/// Replays `solution` on `s`; returns the final state if every move is legal.
pub fn replay(s: &State, solution: &[Move]) -> Option<State> {
    solution.iter().try_fold(*s, |cur, &m| {
        crate::moves::apply(&cur, m).ok().map(|(next, _)| next)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe(State);

    impl KeySearch for Probe {
        type Output = State;
        fn run<K: Key>(self, bits: u32) -> State {
            K::pack(&self.0, bits).unpack(self.0.params(), bits)
        }
    }

    #[test]
    fn key_round_trip_all_widths() {
        let params = [
            Params {
                n_colors: 2,
                capacity: 2,
                n_empty: 1,
            },
            Params {
                n_colors: 12,
                capacity: 4,
                n_empty: 2,
            },
            Params {
                n_colors: 16,
                capacity: 8,
                n_empty: 0,
            },
            Params {
                n_colors: 13,
                capacity: 5,
                n_empty: 2,
            },
        ];
        for p in params {
            let mut units = State::sorted_units(p);
            units.reverse();
            units.rotate_left(3);
            let s = State::from_fill(p, &units).unwrap();
            let bits = cell_bits(p);
            assert_eq!(<[u64; 10]>::pack(&s, bits).unpack(p, bits), s);
            assert_eq!(with_key(p, Probe(s)), s);
        }
    }

    #[test]
    fn heuristic_values() {
        let p = Params {
            n_colors: 3,
            capacity: 3,
            n_empty: 1,
        };
        assert_eq!(heuristic(&State::solved(p).unwrap()), 0);
        let s = State::from_fill(p, &[0, 1, 2, 1, 2, 0, 2, 0, 1]).unwrap();
        assert_eq!(heuristic(&s), 6);
    }
}
