//! Move rules: forward pours, the action mask, and solver-only pruning.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::state::{State, to_u8};

/// A single pour from tube `from` into tube `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Move {
    pub from: u8,
    pub to: u8,
}

/// Why a pour is not allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MoveError {
    #[error("tube index out of range")]
    OutOfRange,
    #[error("cannot pour a tube into itself")]
    SameTube,
    #[error("source tube is empty")]
    SourceEmpty,
    #[error("target tube is full")]
    TargetFull,
    #[error("top colors differ")]
    ColorMismatch,
}

impl Move {
    pub const fn new(from: u8, to: u8) -> Self {
        Self { from, to }
    }

    /// Index into the action mask: `from * n_tubes + to`.
    pub fn action_index(self, n_tubes: usize) -> usize {
        usize::from(self.from) * n_tubes + usize::from(self.to)
    }

    /// Inverse of [`Move::action_index`]. Returns `None` if `index >= n_tubes²`.
    pub fn from_action_index(index: usize, n_tubes: usize) -> Option<Self> {
        (index < n_tubes * n_tubes)
            .then(|| Self::new(to_u8(index / n_tubes), to_u8(index % n_tubes)))
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}->{}", self.from, self.to)
    }
}

/// Checks a pour against the rules.
///
/// # Errors
///
/// Returns the first rule the move breaks.
pub fn check(s: &State, m: Move) -> Result<(), MoveError> {
    let (from, to) = (usize::from(m.from), usize::from(m.to));
    if from >= s.n_tubes() || to >= s.n_tubes() {
        return Err(MoveError::OutOfRange);
    }
    if from == to {
        return Err(MoveError::SameTube);
    }
    let Some(color) = s.top(from) else {
        return Err(MoveError::SourceEmpty);
    };
    if s.is_tube_full(to) {
        return Err(MoveError::TargetFull);
    }
    match s.top(to) {
        Some(c) if c != color => Err(MoveError::ColorMismatch),
        _ => Ok(()),
    }
}

/// Whether the pour is legal.
pub fn is_legal(s: &State, m: Move) -> bool {
    check(s, m).is_ok()
}

/// Applies a pour and returns the new state and the number of units moved.
///
/// All contiguous same-colored units at the top of `from` move, as many as fit in `to`.
///
/// # Errors
///
/// Returns why the move is illegal; the state is not changed.
pub fn apply(s: &State, m: Move) -> Result<(State, u8), MoveError> {
    check(s, m)?;
    Ok(apply_unchecked(s, m))
}

/// [`apply`] without the legality check. The caller guarantees the move is legal.
pub(crate) fn apply_unchecked(s: &State, m: Move) -> (State, u8) {
    let (from, to) = (usize::from(m.from), usize::from(m.to));
    let k = s.top_run(from).min(s.free(to));
    let color = s.tube(from)[usize::from(s.height(from)) - 1];
    let mut next = *s;
    next.pop_units(from, k);
    next.push_units(to, color, k);
    (next, k)
}

/// All legal pours in `(from, to)` lexicographic order.
pub fn legal_moves(s: &State) -> impl Iterator<Item = Move> + '_ {
    let n = to_u8(s.n_tubes());
    (0..n)
        .flat_map(move |from| (0..n).map(move |to| Move::new(from, to)))
        .filter(move |&m| is_legal(s, m))
}

/// Legality of every `(from, to)` pair, indexed by [`Move::action_index`]. Length `n_tubes²`.
pub fn action_mask(s: &State) -> Vec<bool> {
    let n = s.n_tubes();
    (0..n * n)
        .map(|i| Move::from_action_index(i, n).is_some_and(|m| is_legal(s, m)))
        .collect()
}

/// The legal pours the solver explores, in `(from, to)` order.
///
/// Two kinds of pours are skipped because they lead to a state that is a tube permutation of one
/// already reachable, so they never shorten a solution:
///
/// - pouring a single-colored tube entirely into an empty tube (it only swaps the two tubes);
/// - pouring into any empty tube other than the lowest-indexed one (empty tubes are
///   interchangeable).
///
/// Never used by the game itself.
pub(crate) fn solver_moves(s: &State, out: &mut Vec<Move>) {
    out.clear();
    let n = s.n_tubes();
    let first_empty = (0..n).find(|&i| s.is_tube_empty(i));
    for from in 0..n {
        if s.is_tube_empty(from) {
            continue;
        }
        let uniform = s.is_tube_uniform(from);
        for to in 0..n {
            if s.is_tube_empty(to) && (uniform || Some(to) != first_empty) {
                continue;
            }
            let m = Move::new(to_u8(from), to_u8(to));
            if is_legal(s, m) {
                out.push(m);
            }
        }
    }
}

/// An un-pour: take the top `count` units of `from` and put them on `to`.
///
/// Valid reverse moves are exactly those whose matching forward pour (`to` → `from`) is legal in
/// the resulting state and moves exactly `count` units back, so
/// `apply(unapply(s, r), r.forward()) == (s, r.count)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReverseMove {
    pub from: u8,
    pub to: u8,
    pub count: u8,
}

/// The reverse move is not valid in the given state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid reverse move {from}->{to} x{count}", from = .0.from, to = .0.to, count = .0.count)]
pub struct InvalidReverseMove(pub ReverseMove);

impl ReverseMove {
    pub const fn new(from: u8, to: u8, count: u8) -> Self {
        Self { from, to, count }
    }

    /// The forward pour that undoes this reverse move.
    pub const fn forward(self) -> Move {
        Move::new(self.to, self.from)
    }
}

/// Whether `r` is a valid reverse move in `s`.
///
/// With `c` the top color of `from`, `run` its top run length and `free` the free space of `to`:
///
/// 1. `from != to`, `from` non-empty, and `1 <= count <= min(run, free)`;
/// 2. `count < run`, or `from` holds nothing but that run (otherwise the pour back would land on
///    a different color);
/// 3. `to` is empty or its top color is not `c`, or `from` is full (otherwise the pour back
///    would move more than `count` units).
pub fn is_valid_reverse(s: &State, r: ReverseMove) -> bool {
    let (from, to) = (usize::from(r.from), usize::from(r.to));
    if from >= s.n_tubes() || to >= s.n_tubes() || from == to {
        return false;
    }
    let Some(color) = s.top(from) else {
        return false;
    };
    let run = s.top_run(from);
    if r.count == 0 || r.count > run.min(s.free(to)) {
        return false;
    }
    if r.count == run && s.height(from) != run {
        return false;
    }
    s.top(to) != Some(color) || s.is_tube_full(from)
}

/// All valid reverse moves in `(from, to, count)` lexicographic order.
pub fn reverse_moves(s: &State) -> Vec<ReverseMove> {
    let n = to_u8(s.n_tubes());
    let mut out = Vec::new();
    for from in 0..n {
        let max = s.top_run(from.into());
        for to in 0..n {
            for count in 1..=max {
                let r = ReverseMove::new(from, to, count);
                if is_valid_reverse(s, r) {
                    out.push(r);
                }
            }
        }
    }
    out
}

/// Whether `s` has a predecessor: some pour leads to `s`, i.e. [`reverse_moves`] is non-empty.
///
/// A state where every non-empty tube shows a single top unit (or a run that cannot be split off)
/// on a color no other tube can take back has none: a pour always moves the whole top run that
/// fits, so no pour ends there. Stops at the first valid reverse move.
pub fn has_reverse_move(s: &State) -> bool {
    let n = to_u8(s.n_tubes());
    (0..n).any(|from| {
        let max = s.top_run(from.into());
        (0..n)
            .any(|to| (1..=max).any(|count| is_valid_reverse(s, ReverseMove::new(from, to, count))))
    })
}

/// Applies a reverse move.
///
/// # Errors
///
/// Returns [`InvalidReverseMove`] if `r` is not valid in `s`; the state is not changed.
pub fn unapply(s: &State, r: ReverseMove) -> Result<State, InvalidReverseMove> {
    if !is_valid_reverse(s, r) {
        return Err(InvalidReverseMove(r));
    }
    let (from, to) = (usize::from(r.from), usize::from(r.to));
    let color = s.tube(from)[usize::from(s.height(from)) - 1];
    let mut prev = *s;
    prev.pop_units(from, r.count);
    prev.push_units(to, color, r.count);
    Ok(prev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    const P: Params = Params {
        n_colors: 3,
        capacity: 4,
        n_empty: 2,
    };

    fn st(tubes: &[&[u8]]) -> State {
        State::from_tubes(P, tubes).unwrap()
    }

    fn sample() -> State {
        st(&[&[0, 1, 1], &[2, 1], &[0, 0, 2, 2], &[1, 0], &[2]])
    }

    #[test]
    fn every_legality_branch() {
        let s = sample();
        assert_eq!(check(&s, Move::new(0, 9)), Err(MoveError::OutOfRange));
        assert_eq!(check(&s, Move::new(9, 0)), Err(MoveError::OutOfRange));
        assert_eq!(check(&s, Move::new(1, 1)), Err(MoveError::SameTube));
        let with_empty = State::from_tubes(
            Params { n_empty: 3, ..P },
            &[&[0, 1, 1][..], &[2, 1], &[0, 0, 2, 2], &[1, 0], &[2], &[]],
        )
        .unwrap();
        assert_eq!(
            check(&with_empty, Move::new(5, 0)),
            Err(MoveError::SourceEmpty)
        );
        assert_eq!(check(&s, Move::new(0, 2)), Err(MoveError::TargetFull));
        assert_eq!(check(&s, Move::new(0, 3)), Err(MoveError::ColorMismatch));
        assert_eq!(check(&s, Move::new(0, 1)), Ok(()));
        assert_eq!(check(&s, Move::new(2, 4)), Ok(()));
        assert_eq!(apply(&s, Move::new(0, 3)), Err(MoveError::ColorMismatch));
    }

    #[test]
    fn whole_run_moves() {
        let s = sample();
        let (next, k) = apply(&s, Move::new(0, 1)).unwrap();
        assert_eq!(k, 2);
        assert_eq!(next.tube(0), &[0]);
        assert_eq!(next.tube(1), &[2, 1, 1, 1]);

        let s = st(&[&[0, 0, 0], &[2, 2, 2, 2], &[0, 1, 1, 1], &[1], &[]]);
        let (next, k) = apply(&s, Move::new(2, 3)).unwrap();
        assert_eq!(k, 3);
        assert_eq!(next.tube(2), &[0]);
        assert_eq!(next.tube(3), &[1, 1, 1, 1]);
    }

    #[test]
    fn partial_pour() {
        // A run of 2 onto a tube with one free slot: only one unit moves.
        let s = st(&[&[0, 1, 1], &[2, 2, 1], &[0, 0, 0], &[2, 2], &[1]]);
        let (next, k) = apply(&s, Move::new(0, 1)).unwrap();
        assert_eq!(k, 1);
        assert_eq!(next.tube(0), &[0, 1]);
        assert_eq!(next.tube(1), &[2, 2, 1, 1]);
    }

    #[test]
    fn source_becomes_empty() {
        let s = st(&[&[1, 1, 1], &[2, 2, 2, 2], &[0, 0, 0, 0], &[1], &[]]);
        let (next, k) = apply(&s, Move::new(0, 3)).unwrap();
        assert_eq!(k, 3);
        assert!(next.is_tube_empty(0));
        assert!(next.is_solved());
    }

    #[test]
    fn pour_onto_empty() {
        let s = st(&[&[0, 1, 1], &[2, 2, 2, 2], &[0, 0, 0], &[1, 1], &[]]);
        let (next, k) = apply(&s, Move::new(0, 4)).unwrap();
        assert_eq!(k, 2);
        assert_eq!(next.tube(4), &[1, 1]);
        assert_eq!(next.tube(0), &[0]);
    }

    #[test]
    fn legal_moves_order_and_mask() {
        let s = sample();
        let moves: Vec<Move> = legal_moves(&s).collect();
        let mut sorted = moves.clone();
        sorted.sort_unstable();
        assert_eq!(moves, sorted);
        assert!(moves.iter().all(|&m| is_legal(&s, m)));
        let mask = action_mask(&s);
        assert_eq!(mask.len(), 25);
        assert_eq!(mask.iter().filter(|&&b| b).count(), moves.len());
        for m in &moves {
            assert!(mask[m.action_index(5)]);
        }
        assert_eq!(Move::from_action_index(7, 5), Some(Move::new(1, 2)));
        assert_eq!(Move::from_action_index(25, 5), None);
    }

    #[test]
    fn solver_pruning() {
        let s = st(&[&[0, 1, 1], &[2, 2, 2, 2], &[0, 0, 0], &[1, 1], &[]]);
        let s = State::from_tubes(
            Params { n_empty: 3, ..P },
            &[s.tube(0), s.tube(1), s.tube(2), s.tube(3), &[], &[]],
        )
        .unwrap();
        let mut out = Vec::new();
        solver_moves(&s, &mut out);
        // Only the first empty tube (4) is a target; uniform tubes 1, 2, 3 never pour into it.
        assert!(out.iter().all(|m| m.to != 5));
        assert!(out.contains(&Move::new(0, 4)));
        assert!(!out.contains(&Move::new(1, 4)));
        assert!(!out.contains(&Move::new(3, 4)));
        assert!(out.contains(&Move::new(0, 3)));
        assert!(out.iter().all(|&m| is_legal(&s, m)));
    }

    /// Moves `count` top units from `from` to `to` without any rule checks, then tests whether
    /// the forward pour back restores `s` exactly. `None` if the transfer is physically
    /// impossible.
    fn round_trips(s: &State, r: ReverseMove) -> Option<bool> {
        let (from, to, k) = (usize::from(r.from), usize::from(r.to), usize::from(r.count));
        let mut tubes = s.tubes();
        if from == to || k == 0 || k > usize::from(s.top_run(from)) {
            return None;
        }
        let at = tubes[from].len() - k;
        let moved = tubes[from].split_off(at);
        tubes[to].extend(moved);
        let prev = State::from_tubes(s.params(), &tubes).ok()?;
        Some(apply(&prev, r.forward()) == Ok((*s, r.count)))
    }

    #[test]
    fn reverse_conditions() {
        let s = sample();
        // Condition 1.
        assert!(!is_valid_reverse(&s, ReverseMove::new(0, 0, 1)));
        assert!(!is_valid_reverse(&s, ReverseMove::new(0, 4, 0)));
        assert!(!is_valid_reverse(&s, ReverseMove::new(0, 4, 3)));
        assert!(
            !is_valid_reverse(&s, ReverseMove::new(0, 2, 1)),
            "target full"
        );
        // Condition 2: taking the whole run of 1s would expose the 0 below.
        assert!(!is_valid_reverse(&s, ReverseMove::new(0, 4, 2)));
        assert!(is_valid_reverse(&s, ReverseMove::new(0, 4, 1)));
        // ... but a tube holding only the run may be emptied.
        assert!(is_valid_reverse(&s, ReverseMove::new(4, 3, 1)));
        // Condition 3: onto the same color, the pour back would take more units.
        assert!(!is_valid_reverse(&s, ReverseMove::new(0, 1, 1)));
        // ... unless the source is full, which caps the pour back.
        assert!(is_valid_reverse(&s, ReverseMove::new(2, 4, 1)));
        assert_eq!(
            unapply(&s, ReverseMove::new(0, 1, 1)),
            Err(InvalidReverseMove(ReverseMove::new(0, 1, 1)))
        );
    }

    #[test]
    fn reverse_moves_are_exactly_the_round_trips() {
        let s = sample();
        let listed = reverse_moves(&s);
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        assert_eq!(listed, sorted);
        for from in 0..5 {
            for to in 0..5 {
                for count in 0..=4 {
                    let r = ReverseMove::new(from, to, count);
                    assert_eq!(
                        listed.contains(&r),
                        round_trips(&s, r).unwrap_or(false),
                        "{r:?}"
                    );
                    if listed.contains(&r) {
                        assert!(unapply(&s, r).is_ok());
                    }
                }
            }
        }
        assert!(has_reverse_move(&s));
        let prev = unapply(&s, ReverseMove::new(2, 4, 1)).unwrap();
        assert_eq!(prev.tube(2), &[0, 0, 2]);
        assert_eq!(prev.tube(4), &[2, 2]);
    }

    #[test]
    fn has_reverse_move_matches_reverse_moves() {
        // No predecessor: every tube is full with a single top unit on a different color.
        let dead = st(&[&[0, 0, 0, 1], &[1, 1, 1, 2], &[2, 2, 2, 0], &[], &[]]);
        assert_eq!(reverse_moves(&dead), Vec::new());
        assert!(!has_reverse_move(&dead));
        // Every state up to three reverse moves from the solved one.
        let solved = State::solved(P).unwrap();
        let mut frontier = vec![solved];
        for _ in 0..3 {
            let mut next = Vec::new();
            for s in &frontier {
                assert_eq!(has_reverse_move(s), !reverse_moves(s).is_empty());
                for r in reverse_moves(s) {
                    next.push(unapply(s, r).unwrap());
                }
            }
            frontier = next;
        }
        assert_eq!(
            has_reverse_move(&sample()),
            !reverse_moves(&sample()).is_empty()
        );
    }
}
