//! Star rating and the move-counting rules of a play session.

use serde::{Deserialize, Serialize};

use crate::moves::{Move, MoveError, apply};
use crate::state::State;

/// Star thresholds as per-mille fractions of `opt` (D5). Integers avoid floating-point `ceil`
/// errors such as `0.10 * 30.0 = 3.0000000000000004`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StarConfig {
    pub c4_permille: u32,
    pub c3_permille: u32,
    pub c2_permille: u32,
}

impl Default for StarConfig {
    fn default() -> Self {
        Self {
            c4_permille: 100,
            c3_permille: 250,
            c2_permille: 500,
        }
    }
}

/// The player claims fewer moves than the optimum, which means the solver is wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StarError {
    #[error("player solved in {player} moves, below the optimum {opt} (solver bug)")]
    BelowOptimal { player: u32, opt: u32 },
}

/// `ceil(c / 1000 * opt)` in integer arithmetic.
fn ceil_pm(c: u32, opt: u32) -> u32 {
    let x = (u64::from(c) * u64::from(opt)).div_ceil(1000);
    u32::try_from(x).unwrap_or(u32::MAX)
}

/// Maximum extra moves for 4, 3 and 2 stars: `(t4, t3, t2)`, strictly increasing.
pub fn thresholds(opt: u32, cfg: &StarConfig) -> (u32, u32, u32) {
    let t4 = ceil_pm(cfg.c4_permille, opt).max(1);
    let t3 = ceil_pm(cfg.c3_permille, opt).max(t4.saturating_add(1));
    let t2 = ceil_pm(cfg.c2_permille, opt).max(t3.saturating_add(1));
    (t4, t3, t2)
}

/// Stars (1–5) for solving a puzzle with optimum `opt` in `player_moves` pours.
///
/// # Errors
///
/// [`StarError::BelowOptimal`] if `player_moves < opt`.
pub fn stars(player_moves: u32, opt: u32, cfg: &StarConfig) -> Result<u8, StarError> {
    let extra = player_moves
        .checked_sub(opt)
        .ok_or(StarError::BelowOptimal {
            player: player_moves,
            opt,
        })?;
    let (t4, t3, t2) = thresholds(opt, cfg);
    Ok(match extra {
        0 => 5,
        e if e <= t4 => 4,
        e if e <= t3 => 3,
        e if e <= t2 => 2,
        _ => 1,
    })
}

/// One player's attempt at a puzzle.
///
/// The counted value is the total number of pours made: [`Session::undo`] does not decrement it
/// and [`Session::restart`] does not reset it. Web and Python both use this type so the rule is
/// implemented once.
#[derive(Clone, Debug)]
pub struct Session {
    initial: State,
    current: State,
    undo_stack: Vec<State>,
    moves_made: u32,
}

impl Session {
    pub fn new(initial: State) -> Self {
        Self {
            initial,
            current: initial,
            undo_stack: Vec::new(),
            moves_made: 0,
        }
    }

    pub const fn initial(&self) -> &State {
        &self.initial
    }

    pub const fn state(&self) -> &State {
        &self.current
    }

    /// Total pours made on this puzzle, including undone ones and those before a restart.
    pub const fn moves_made(&self) -> u32 {
        self.moves_made
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn is_solved(&self) -> bool {
        self.current.is_solved()
    }

    /// Plays a pour and returns the number of units moved.
    ///
    /// # Errors
    ///
    /// An illegal move is rejected, not counted, and leaves the session unchanged.
    pub fn play(&mut self, m: Move) -> Result<u8, MoveError> {
        let (next, k) = apply(&self.current, m)?;
        self.undo_stack.push(self.current);
        self.current = next;
        self.moves_made = self.moves_made.saturating_add(1);
        Ok(k)
    }

    /// Reverts the last pour since the start or the last restart. Returns `false` if there is
    /// nothing to undo. The move counter is not decremented.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(prev) => {
                self.current = prev;
                true
            }
            None => false,
        }
    }

    /// Returns to the initial state and clears the undo history. The move counter is kept.
    pub fn restart(&mut self) {
        self.current = self.initial;
        self.undo_stack.clear();
    }

    /// Stars for the current session, or `None` if the puzzle is not solved yet.
    pub fn stars(&self, opt: u32, cfg: &StarConfig) -> Option<Result<u8, StarError>> {
        self.is_solved().then(|| stars(self.moves_made, opt, cfg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    fn s(player: u32, opt: u32) -> u8 {
        stars(player, opt, &StarConfig::default()).unwrap()
    }

    #[test]
    fn opt_30_uses_exact_integer_ceil() {
        // Floating point would give t4 = ceil(3.0000000000000004) = 4.
        assert_eq!(thresholds(30, &StarConfig::default()), (3, 8, 15));
        let table = [
            (30, 5),
            (31, 4),
            (33, 4),
            (34, 3),
            (38, 3),
            (39, 2),
            (45, 2),
            (46, 1),
            (1000, 1),
        ];
        for (player, expected) in table {
            assert_eq!(s(player, 30), expected, "player = {player}");
        }
    }

    #[test]
    fn opt_1_thresholds_are_strictly_increasing() {
        assert_eq!(thresholds(1, &StarConfig::default()), (1, 2, 3));
        let table = [(1, 5), (2, 4), (3, 3), (4, 2), (5, 1)];
        for (player, expected) in table {
            assert_eq!(s(player, 1), expected, "player = {player}");
        }
    }

    #[test]
    fn other_thresholds() {
        assert_eq!(thresholds(0, &StarConfig::default()), (1, 2, 3));
        assert_eq!(thresholds(10, &StarConfig::default()), (1, 3, 5));
        assert_eq!(thresholds(11, &StarConfig::default()), (2, 3, 6));
        assert_eq!(thresholds(100, &StarConfig::default()), (10, 25, 50));
        let cfg = StarConfig {
            c4_permille: 0,
            c3_permille: 0,
            c2_permille: 0,
        };
        assert_eq!(thresholds(100, &cfg), (1, 2, 3));
        assert_eq!(s(0, 0), 5);
    }

    #[test]
    fn below_optimal_is_an_error() {
        assert_eq!(
            stars(9, 10, &StarConfig::default()),
            Err(StarError::BelowOptimal { player: 9, opt: 10 })
        );
    }

    #[test]
    fn session_counts_every_pour() {
        let p = Params {
            n_colors: 2,
            capacity: 2,
            n_empty: 1,
        };
        let initial = State::from_tubes(p, &[&[0, 1][..], &[1, 0], &[]]).unwrap();
        let mut session = Session::new(initial);
        assert!(!session.undo());
        assert_eq!(session.play(Move::new(0, 0)), Err(MoveError::SameTube));
        assert_eq!(session.moves_made(), 0);

        session.play(Move::new(0, 2)).unwrap();
        assert!(session.undo());
        assert_eq!(session.state(), &initial);
        assert_eq!(session.moves_made(), 1, "undo does not decrement");

        session.play(Move::new(0, 2)).unwrap();
        session.restart();
        assert_eq!(session.state(), &initial);
        assert!(!session.can_undo());
        assert_eq!(session.moves_made(), 2, "restart does not reset");
        assert_eq!(session.stars(3, &StarConfig::default()), None);

        // An optimal solution takes 3 pours.
        for m in [Move::new(0, 2), Move::new(1, 0), Move::new(2, 1)] {
            session.play(m).unwrap();
        }
        assert!(session.is_solved());
        assert_eq!(session.moves_made(), 5);
        // opt = 3, extra = 2 > t4 = 1, <= t3 = 2 -> 3 stars.
        assert_eq!(session.stars(3, &StarConfig::default()), Some(Ok(3)));
        assert_eq!(
            session.stars(6, &StarConfig::default()),
            Some(Err(StarError::BelowOptimal { player: 5, opt: 6 }))
        );
    }
}
