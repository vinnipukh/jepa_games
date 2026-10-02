//! The `Generator` trait (D7) and the validation step every generator shares.

use serde::{Deserialize, Serialize};

use crate::metrics::DifficultyMetrics;
use crate::moves::Move;
use crate::params::{Params, ParamsError};
use crate::solver::{SolveResult, SolverLimits, solve};
use crate::state::State;

/// A seeded puzzle generator. `(ID, VERSION, variant, params, seed, GenConfig)` fully determines
/// the generated puzzle.
pub trait Generator {
    /// Generator family, e.g. `"uniform"`, `"turan"`.
    const ID: &'static str;
    /// Bumped whenever the algorithm changes what a seed produces.
    const VERSION: u32;

    /// Human-readable sub-type, recorded with every puzzle, e.g. `"fisher_yates"`,
    /// `"scramble(steps=40)"`.
    fn variant(&self) -> String;

    /// A new seed when the caller supplies none: OS entropy for uniform, time + counter for
    /// turan. The caller reads the clock; core never does (D1).
    ///
    /// # Errors
    ///
    /// [`GenError::Entropy`] if the seed source fails.
    fn fresh_seed(&self, now_nanos: u64) -> Result<u64, GenError>;

    /// Generates one puzzle.
    ///
    /// # Errors
    ///
    /// Invalid params, or no acceptable puzzle within `cfg.max_attempts`.
    fn generate(
        &self,
        params: &Params,
        seed: u64,
        cfg: &GenConfig,
    ) -> Result<GeneratedPuzzle, GenError>;
}

/// Generation settings. Part of the reproducibility key: changing `max_states` can change which
/// attempt is accepted, so it is stored with every record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenConfig {
    /// Reject puzzles whose optimum is below this.
    pub min_opt: u32,
    /// Give up after this many rejected attempts.
    pub max_attempts: u32,
    /// Solver state-count limit. Generation never uses a time limit (D11).
    pub max_states: u64,
    pub metrics: MetricsConfig,
}

impl Default for GenConfig {
    fn default() -> Self {
        Self {
            min_opt: 1,
            max_attempts: 10_000,
            max_states: SolverLimits::DEFAULT_MAX_STATES,
            metrics: MetricsConfig::default(),
        }
    }
}

impl GenConfig {
    /// The solver limits generation uses: state count only (D11).
    pub const fn solver_limits(&self) -> SolverLimits {
        SolverLimits::states(self.max_states)
    }
}

/// Which difficulty metrics to compute, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Number of random legal-move rollouts (0 disables them).
    pub random_rollouts: u32,
    /// A rollout stops after `rollout_cap_factor * opt_moves` moves (D4: k = 4).
    pub rollout_cap_factor: u32,
    /// Compute the depth-1 and depth-2 dead-end ratios (expensive: dozens of solves).
    pub dead_end_ratios: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            random_rollouts: 64,
            rollout_cap_factor: 4,
            dead_end_ratios: false,
        }
    }
}

/// Everything recorded about one generated puzzle.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeneratedPuzzle {
    pub state: State,
    pub seed: u64,
    pub generator_id: &'static str,
    pub generator_version: u32,
    pub generator_variant: String,
    pub opt_moves: u32,
    pub solution: Vec<Move>,
    pub metrics: DifficultyMetrics,
    pub canonical_hash: u64,
    /// Compact encoding of params + initial state (see [`crate::puzzle_code`]).
    pub puzzle_code: String,
    /// Attempts used, including the accepted one.
    pub attempts: u32,
}

/// Why generation failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenError {
    #[error(transparent)]
    InvalidParams(#[from] ParamsError),
    #[error("no acceptable puzzle within {attempts} attempts")]
    TooManyAttempts { attempts: u32 },
    #[error("seed source failed: {0}")]
    Entropy(String),
}

/// Why a candidate state was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    AlreadySolved,
    Unsolvable,
    /// The solver hit `max_states`.
    Timeout,
    BelowMinOpt {
        opt_moves: u32,
    },
}

/// A candidate that passed validation, with its optimal solution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accepted {
    pub opt_moves: u32,
    pub solution: Vec<Move>,
    pub states_expanded: u64,
}

/// The validation every generator applies to a candidate: reject solved states, solve with the
/// state-count limit only, reject `Unsolvable`, `Timeout` and `opt_moves < min_opt`.
///
/// # Errors
///
/// The reason the candidate is rejected.
pub fn evaluate(state: &State, cfg: &GenConfig) -> Result<Accepted, Rejection> {
    if state.is_solved() {
        return Err(Rejection::AlreadySolved);
    }
    match solve(state, &cfg.solver_limits()) {
        SolveResult::Solvable { opt_moves, .. } if opt_moves < cfg.min_opt => {
            Err(Rejection::BelowMinOpt { opt_moves })
        }
        SolveResult::Solvable {
            opt_moves,
            solution,
            states_expanded,
        } => Ok(Accepted {
            opt_moves,
            solution,
            states_expanded,
        }),
        SolveResult::Unsolvable { .. } => Err(Rejection::Unsolvable),
        SolveResult::Timeout { .. } => Err(Rejection::Timeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Params = Params {
        n_colors: 3,
        capacity: 3,
        n_empty: 1,
    };

    #[test]
    fn evaluate_rejections() {
        let cfg = GenConfig::default();
        assert_eq!(
            evaluate(&State::solved(P).unwrap(), &cfg),
            Err(Rejection::AlreadySolved)
        );
        let unsolvable =
            State::from_tubes(P, &[&[0, 2, 1][..], &[0, 1, 1], &[0, 2, 2], &[]]).unwrap();
        assert_eq!(evaluate(&unsolvable, &cfg), Err(Rejection::Unsolvable));
        let one_move = State::from_tubes(P, &[&[0, 0][..], &[1, 1, 1], &[2, 2, 2], &[0]]).unwrap();
        assert_eq!(evaluate(&one_move, &cfg).unwrap().opt_moves, 1);
        let strict = GenConfig { min_opt: 2, ..cfg };
        assert_eq!(
            evaluate(&one_move, &strict),
            Err(Rejection::BelowMinOpt { opt_moves: 1 })
        );
        let tiny = GenConfig {
            max_states: 1,
            ..cfg
        };
        assert_eq!(evaluate(&unsolvable, &tiny), Err(Rejection::Timeout));
    }

    #[test]
    fn config_serializes() {
        let cfg = GenConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<GenConfig>(&json).unwrap(), cfg);
        assert_eq!(cfg.solver_limits().max_time, None);
    }
}
