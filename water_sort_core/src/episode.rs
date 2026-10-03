//! Episode rules of the RL environment (Phase 5.2): reward, illegal actions, termination and
//! truncation. The Python `WaterSortEnv`, its native vector env and the golden rollouts all call
//! [`episode_step`], so the rules exist once.
//!
//! - An action is an index `from * n_tubes + to` ([`Move::action_index`]).
//! - An illegal action leaves the state unchanged and is reported, never raised: unmasked agents
//!   and Gymnasium's `check_env` must not crash. It still costs a move and counts towards the
//!   move limit.
//! - Reward: −1 per action, plus optional potential-based shaping `γ·Φ(s') − Φ(s)` with
//!   `Φ = −color_changes` (Ng et al. 1999), applied to every transition, illegal self-loops
//!   included, so the optimal policy is unchanged. A solved state has `Φ = 0`.
//! - Termination: solved, or a dead end (not solved and no legal move; optionally also a state
//!   the solver proves unsolvable). Truncation: not terminated and `moves_so_far` reached the
//!   move limit `k · opt_moves` (D4).

use crate::moves::{Move, apply, legal_moves};
use crate::solver::{SolveResult, SolverLimits, solve};
use crate::state::State;

/// The per-episode settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EpisodeRules {
    /// Truncate once this many actions were taken ([`move_limit`]).
    pub move_limit: u32,
    /// Discount for potential-based shaping, or `None` for the plain −1 reward.
    pub shaping_gamma: Option<f64>,
    /// Also end the episode when the solver proves the state unsolvable, searching at most this
    /// many states (a solver timeout is not a dead end). Expensive: one solve per step.
    pub dead_end_max_states: Option<u64>,
}

/// The move limit `k · opt_moves` (D4), saturating.
pub const fn move_limit(k: u32, opt_moves: u32) -> u32 {
    k.saturating_mul(opt_moves)
}

/// The shaping potential `Φ(s) = −color_changes(s)`.
pub fn potential(s: &State) -> f64 {
    -f64::from(s.color_changes())
}

/// The outcome of one environment step.
// The flags are the Gymnasium step outputs and `info` entries, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EpisodeStep {
    pub state: State,
    /// Units poured; 0 for an illegal action.
    pub units_moved: u8,
    pub reward: f64,
    pub illegal: bool,
    pub solved: bool,
    pub dead_end: bool,
    pub terminated: bool,
    pub truncated: bool,
    /// Actions taken in the episode, including this one and illegal ones.
    pub moves_so_far: u32,
}

/// The action index is not below `n_tubes²`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("action {action} is out of range for {n_tubes} tubes")]
pub struct ActionOutOfRange {
    pub action: usize,
    pub n_tubes: usize,
}

/// Whether `s` ends an episode as a dead end: not solved and no legal move, or (with
/// `dead_end_max_states`) proven unsolvable by the solver.
pub fn is_dead_end(s: &State, dead_end_max_states: Option<u64>) -> bool {
    if s.is_solved() {
        return false;
    }
    if legal_moves(s).next().is_none() {
        return true;
    }
    dead_end_max_states.is_some_and(|max_states| {
        matches!(
            solve(s, &SolverLimits::states(max_states)),
            SolveResult::Unsolvable { .. }
        )
    })
}

/// Takes action `action` in `state`, after `moves_so_far` earlier actions of the episode.
///
/// # Errors
///
/// [`ActionOutOfRange`] if `action >= n_tubes²`.
pub fn episode_step(
    state: &State,
    action: usize,
    moves_so_far: u32,
    rules: &EpisodeRules,
) -> Result<EpisodeStep, ActionOutOfRange> {
    let n_tubes = state.n_tubes();
    let m = Move::from_action_index(action, n_tubes).ok_or(ActionOutOfRange { action, n_tubes })?;
    let (next, units_moved, illegal) = match apply(state, m) {
        Ok((next, k)) => (next, k, false),
        Err(_) => (*state, 0, true),
    };
    // Exactly `-1 + (γ·Φ(s') − Φ(s))` in f64, the formula every target reproduces.
    let reward = rules.shaping_gamma.map_or(-1.0, |gamma| {
        -1.0 + (gamma * potential(&next) - potential(state))
    });
    let moves_so_far = moves_so_far.saturating_add(1);
    let solved = next.is_solved();
    let dead_end = !solved && is_dead_end(&next, rules.dead_end_max_states);
    let terminated = solved || dead_end;
    Ok(EpisodeStep {
        state: next,
        units_moved,
        reward,
        illegal,
        solved,
        dead_end,
        terminated,
        truncated: !terminated && moves_so_far >= rules.move_limit,
        moves_so_far,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    const fn rules(move_limit: u32) -> EpisodeRules {
        EpisodeRules {
            move_limit,
            shaping_gamma: None,
            dead_end_max_states: None,
        }
    }

    fn st(capacity: u8, tubes: &[&[u8]]) -> State {
        let units: usize = tubes.iter().map(|t| t.len()).sum();
        let n_colors = u8::try_from(units / usize::from(capacity)).unwrap();
        let params = Params {
            n_colors,
            capacity,
            n_empty: u8::try_from(tubes.len()).unwrap() - n_colors,
        };
        State::from_tubes(params, tubes).unwrap()
    }

    #[test]
    fn legal_move_costs_one() {
        let s = st(2, &[&[0, 1], &[1, 0], &[]]);
        let out = episode_step(&s, 2, 0, &rules(8)).unwrap();
        assert_eq!(out.state, apply(&s, Move::new(0, 2)).unwrap().0);
        assert_eq!(out.units_moved, 1);
        assert!((out.reward + 1.0).abs() < f64::EPSILON);
        assert!(!out.illegal && !out.terminated && !out.truncated);
        assert_eq!(out.moves_so_far, 1);
    }

    #[test]
    fn illegal_action_keeps_the_state_and_counts() {
        let s = st(2, &[&[0, 1], &[1, 0], &[]]);
        let out = episode_step(&s, 1, 3, &rules(4)).unwrap();
        assert_eq!(out.state, s);
        assert!(out.illegal);
        assert_eq!(out.units_moved, 0);
        assert_eq!(out.moves_so_far, 4);
        assert!(out.truncated && !out.terminated);
        assert_eq!(
            episode_step(&s, 9, 0, &rules(4)),
            Err(ActionOutOfRange {
                action: 9,
                n_tubes: 3
            })
        );
    }

    #[test]
    fn solved_terminates_before_truncation() {
        let s = st(2, &[&[0, 0], &[1], &[1]]);
        let out = episode_step(&s, 7, 0, &rules(1)).unwrap();
        assert!(out.solved && out.terminated && !out.truncated && !out.dead_end);
    }

    #[test]
    fn dead_end_without_legal_moves() {
        let s = st(2, &[&[0, 2], &[1, 2], &[0, 1], &[]]);
        let out = episode_step(&s, 2 * 4 + 3, 0, &rules(100)).unwrap();
        assert_eq!(
            out.state.tubes(),
            vec![vec![0, 2], vec![1, 2], vec![0], vec![1]]
        );
        assert!(out.dead_end && out.terminated && !out.solved);
        assert!(is_dead_end(&out.state, None));
    }

    #[test]
    fn solver_dead_end_is_opt_in() {
        // Legal moves remain (into the empty tube), but the position is unsolvable.
        let s = st(3, &[&[], &[0, 1, 1], &[0, 2, 2], &[0, 1, 2]]);
        assert!(legal_moves(&s).next().is_some());
        assert!(!is_dead_end(&s, None));
        assert!(matches!(
            solve(&s, &SolverLimits::states(10_000)),
            SolveResult::Unsolvable { .. }
        ));
        assert!(is_dead_end(&s, Some(10_000)));
        // A timeout is not a dead end.
        assert!(!is_dead_end(&s, Some(1)));
    }

    #[test]
    fn shaping_is_potential_based() {
        let s = st(2, &[&[0, 1], &[1, 0], &[]]);
        let gamma = 0.9;
        let r = EpisodeRules {
            shaping_gamma: Some(gamma),
            ..rules(8)
        };
        let out = episode_step(&s, 2, 0, &r).unwrap();
        // color_changes: 2 before, 1 after.
        let expected = -1.0 - gamma + 2.0;
        assert!((out.reward - expected).abs() < 1e-12);
        let out = episode_step(&s, 1, 0, &r).unwrap();
        let expected = -1.0 - 2.0 * gamma + 2.0;
        assert!(out.illegal && (out.reward - expected).abs() < 1e-12);
        assert!((potential(&State::solved(s.params()).unwrap())).abs() < f64::EPSILON);
    }

    #[test]
    fn move_limit_saturates() {
        assert_eq!(move_limit(4, 20), 80);
        assert_eq!(move_limit(u32::MAX, 2), u32::MAX);
    }
}
