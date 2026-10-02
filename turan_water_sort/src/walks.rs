//! The constructions of [`crate::TuranStrategy::PourWalk`] and
//! [`crate::TuranStrategy::ReverseSearch`] (D16).

use std::collections::HashSet;

use rand_core::Rng;
use water_sort_core::{
    Layout, Move, Params, ReverseMove, State, apply, bounded_u32, fisher_yates, heuristic,
    legal_moves, reverse_moves, unapply,
};

use crate::{has_standard_heights, shuffle_symmetries};

/// One step of a pour walk: a pour, or an un-pour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Pour(Move),
    Unpour(ReverseMove),
}

/// Builds one pour-walk candidate, continuing `rng`'s stream (the construction of
/// [`crate::TuranStrategy::PourWalk`]):
///
/// 1. start from [`State::solved`];
/// 2. apply `steps` steps, each drawn with [`bounded_u32`] from the legal pours ([`legal_moves`])
///    followed by the reverse moves ([`reverse_moves`]), both in their fixed order, excluding the
///    exact undo of the previous step unless nothing else is left: a random walk on the
///    undirected pour graph. Forward pours can leave the solvable set, so the result can be
///    unsolvable or solved; validation rejects those;
/// 3. shuffle the color labels, then all tubes, with [`fisher_yates`] (distributed layout only).
///
/// # Panics
///
/// If `params` is invalid.
pub fn pour_walk<R: Rng + ?Sized>(rng: &mut R, params: Params, steps: u32) -> State {
    let mut state = State::solved(params).expect("valid params");
    let mut undo: Option<Step> = None;
    for _ in 0..steps {
        let all: Vec<Step> = legal_moves(&state)
            .map(Step::Pour)
            .chain(reverse_moves(&state).into_iter().map(Step::Unpour))
            .collect();
        let mut choices: Vec<Step> = all.iter().copied().filter(|&m| Some(m) != undo).collect();
        if choices.is_empty() {
            choices = all;
        }
        if choices.is_empty() {
            break;
        }
        let n = u32::try_from(choices.len()).expect("few moves");
        match choices[bounded_u32(rng, n) as usize] {
            Step::Pour(m) => {
                let (next, k) = apply(&state, m).expect("listed pours are legal");
                state = next;
                undo = Some(Step::Unpour(ReverseMove::new(m.to, m.from, k)));
            }
            Step::Unpour(r) => {
                state = unapply(&state, r).expect("listed reverse moves are valid");
                undo = Some(Step::Pour(r.forward()));
            }
        }
    }
    shuffle_symmetries(rng, &state, Layout::Distributed)
}

/// What one reverse search explored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOutcome {
    /// The best-scoring state (labels and tubes shuffled), or `None` if no state scored above 0.
    pub state: Option<State>,
    /// Its [`reverse_search`] score (0 if none).
    pub score: u64,
    /// Its depth: reverse moves from the solved state along the search path.
    pub depth: u32,
    /// Distinct states visited, the solved state included.
    pub visited: u32,
}

/// One DFS node: a state, its shuffled reverse moves, and the path statistics that reach it.
struct Frame {
    state: State,
    moves: Vec<ReverseMove>,
    next: usize,
    depth: u32,
    switches: u32,
    color: Option<u8>,
}

impl Frame {
    fn new<R: Rng + ?Sized>(
        rng: &mut R,
        state: State,
        depth: u32,
        switches: u32,
        color: Option<u8>,
    ) -> Self {
        let mut moves = reverse_moves(&state);
        fisher_yates(rng, &mut moves);
        Self {
            state,
            moves,
            next: 0,
            depth,
            switches,
            color,
        }
    }
}

/// The search score of a state reached with `switches` color switches (the analog of I2A's
/// `RoomScore = BoxSwaps × Σ BoxDisplacement`): `switches × heuristic(s)`, where
/// [`heuristic`] (`segments − n_colors`) is the solver's lower bound on the moves left. A state
/// with a sorted (full, one-color) tube scores 0, like a room with a box on its target; for
/// [`Layout::Standard`], so does a state without standard heights.
fn score(state: &State, switches: u32, layout: Layout) -> u64 {
    if layout == Layout::Standard && !has_standard_heights(state) {
        return 0;
    }
    let sorted = (0..state.n_tubes()).any(|i| state.is_tube_full(i) && state.is_tube_uniform(i));
    if sorted {
        return 0;
    }
    u64::from(switches) * u64::from(heuristic(state))
}

/// Builds one reverse-search candidate, continuing `rng`'s stream (the construction of
/// [`crate::TuranStrategy::ReverseSearch`], after the Sokoban generator of Racanière et al.
/// 2017 (I2A), itself after Taylor & Parberry 2011):
///
/// 1. depth-first search over reverse moves from [`State::solved`], each node's reverse moves
///    ([`reverse_moves`]) shuffled with [`fisher_yates`]; states already visited are skipped,
///    paths stop at `max_depth` reverse moves, and the search stops after `max_states` distinct
///    states (or when nothing is left);
/// 2. every visited state is scored ([`score`]: color switches along its path × `segments −
///    n_colors`; 0 with a sorted tube or, for `Standard`, without standard heights); the first
///    state with the highest score wins;
/// 3. shuffle the color labels, then the tube order (as for the scramble).
///
/// The result is solvable by construction. If nothing scores above 0, `state` is `None` (a
/// construction rejection).
///
/// # Panics
///
/// If `params` is invalid.
pub fn reverse_search<R: Rng + ?Sized>(
    rng: &mut R,
    params: Params,
    max_depth: u32,
    max_states: u32,
    layout: Layout,
) -> SearchOutcome {
    let root = State::solved(params).expect("valid params");
    let mut visited: HashSet<State> = HashSet::new();
    visited.insert(root);
    let mut best: Option<(u64, u32, State)> = None;
    let mut stack = vec![Frame::new(rng, root, 0, 0, None)];
    while visited.len() < max_states as usize {
        let Some(top) = stack.last_mut() else { break };
        if top.next == top.moves.len() || top.depth == max_depth {
            stack.pop();
            continue;
        }
        let r = top.moves[top.next];
        top.next += 1;
        let child = unapply(&top.state, r).expect("listed reverse moves are valid");
        // Membership only: the set's iteration order never matters (CLAUDE.md rule 3).
        if !visited.insert(child) {
            continue;
        }
        let moved = top.state.top(usize::from(r.from));
        let switches = top.switches + u32::from(top.color.is_some() && top.color != moved);
        let depth = top.depth + 1;
        let s = score(&child, switches, layout);
        if s > best.as_ref().map_or(0, |b| b.0) {
            best = Some((s, depth, child));
        }
        let frame = Frame::new(rng, child, depth, switches, moved);
        stack.push(frame);
    }
    let visited = u32::try_from(visited.len()).expect("at most max_states");
    match best {
        Some((score, depth, state)) => SearchOutcome {
            state: Some(shuffle_symmetries(rng, &state, layout)),
            score,
            depth,
            visited,
        },
        None => SearchOutcome {
            state: None,
            score: 0,
            depth: 0,
            visited,
        },
    }
}
