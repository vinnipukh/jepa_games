//! The `Generator` trait (D7) and the validation step every generator shares.

use rand_core::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::canon::canonical_hash;
use crate::metrics::compute_metrics;
use crate::puzzle_code;

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

    /// Generates one puzzle, reporting every attempt to `observer`.
    ///
    /// The observer only watches: it cannot change which attempt is accepted, so the result is
    /// the same as [`Generator::generate`]'s.
    ///
    /// # Errors
    ///
    /// Invalid params, or no acceptable puzzle within `cfg.max_attempts`.
    fn generate_observed<O: Observer>(
        &self,
        params: &Params,
        seed: u64,
        cfg: &GenConfig,
        observer: &mut O,
    ) -> Result<GeneratedPuzzle, GenError>
    where
        Self: Sized;

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
    ) -> Result<GeneratedPuzzle, GenError>
    where
        Self: Sized,
    {
        self.generate_observed(params, seed, cfg, &mut ())
    }

    /// [`Generator::generate`], plus the number of rejected attempts by reason.
    ///
    /// # Errors
    ///
    /// As [`Generator::generate`]. The counts are returned only on success; use
    /// [`Generator::generate_observed`] to see them for a failed run too.
    fn generate_traced(
        &self,
        params: &Params,
        seed: u64,
        cfg: &GenConfig,
    ) -> Result<(GeneratedPuzzle, RejectionCounts), GenError>
    where
        Self: Sized,
    {
        let mut counts = RejectionCounts::default();
        let puzzle = self.generate_observed(params, seed, cfg, &mut counts)?;
        Ok((puzzle, counts))
    }

    /// Builds the record for an accepted candidate: identity fields, metrics (rollouts use RNG
    /// type `R`, see [`compute_metrics`]), canonical hash and puzzle code.
    fn assemble<R: Rng + SeedableRng>(
        &self,
        state: State,
        seed: u64,
        attempts: u32,
        accepted: Accepted,
        cfg: &GenConfig,
    ) -> GeneratedPuzzle
    where
        Self: Sized,
    {
        GeneratedPuzzle {
            metrics: compute_metrics::<R>(&state, &accepted, cfg),
            canonical_hash: canonical_hash(&state),
            puzzle_code: puzzle_code::encode(&state),
            state,
            seed,
            generator_id: Self::ID,
            generator_version: Self::VERSION,
            generator_variant: self.variant(),
            opt_moves: accepted.opt_moves,
            solution: accepted.solution,
            attempts,
        }
    }
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
    evaluate_counted(state, cfg).outcome
}

/// One validated candidate: the [`evaluate`] outcome and the solver's expanded-state count
/// (0 when no search ran).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    pub outcome: Result<Accepted, Rejection>,
    pub states_expanded: u64,
}

/// [`evaluate`], also reporting how many states the solver expanded, whatever the outcome.
pub fn evaluate_counted(state: &State, cfg: &GenConfig) -> Evaluation {
    if state.is_solved() {
        return Evaluation {
            outcome: Err(Rejection::AlreadySolved),
            states_expanded: 0,
        };
    }
    let result = solve(state, &cfg.solver_limits());
    let states_expanded = result.states_expanded();
    let outcome = match result {
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
    };
    Evaluation {
        outcome,
        states_expanded,
    }
}

/// Watches a generation loop, e.g. to count rejections or time the solver. It only observes, so
/// it never changes which attempt is accepted.
pub trait Observer {
    /// Called just before a candidate is validated.
    fn before_attempt(&mut self) {}
    /// Called with the validation of every candidate, accepted or not.
    fn after_attempt(&mut self, evaluation: &Evaluation);
}

/// Observes nothing.
impl Observer for () {
    fn after_attempt(&mut self, _: &Evaluation) {}
}

/// Rejected attempts by reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RejectionCounts {
    pub already_solved: u32,
    pub unsolvable: u32,
    pub timeout: u32,
    pub below_min_opt: u32,
}

impl RejectionCounts {
    /// Counts one rejection.
    pub const fn record(&mut self, rejection: &Rejection) {
        let slot = match rejection {
            Rejection::AlreadySolved => &mut self.already_solved,
            Rejection::Unsolvable => &mut self.unsolvable,
            Rejection::Timeout => &mut self.timeout,
            Rejection::BelowMinOpt { .. } => &mut self.below_min_opt,
        };
        *slot += 1;
    }

    /// All rejections.
    pub const fn total(&self) -> u32 {
        self.already_solved + self.unsolvable + self.timeout + self.below_min_opt
    }
}

impl Observer for RejectionCounts {
    fn after_attempt(&mut self, evaluation: &Evaluation) {
        if let Err(rejection) = &evaluation.outcome {
            self.record(rejection);
        }
    }
}

/// The rejection loop every generator shares: draws up to `cfg.max_attempts` candidates from
/// `candidate`, validates each with [`evaluate`], and returns the first accepted one with its
/// 1-based attempt number. A rejection simply asks `candidate` for the next one, so a generator
/// whose `candidate` continues one RNG stream is fully determined by its seed.
///
/// # Errors
///
/// [`GenError::TooManyAttempts`] if every candidate is rejected.
pub fn attempt_loop<O: Observer>(
    cfg: &GenConfig,
    observer: &mut O,
    mut candidate: impl FnMut() -> State,
) -> Result<(State, u32, Accepted), GenError> {
    for attempt in 1..=cfg.max_attempts {
        let state = candidate();
        observer.before_attempt();
        let evaluation = evaluate_counted(&state, cfg);
        observer.after_attempt(&evaluation);
        if let Ok(accepted) = evaluation.outcome {
            return Ok((state, attempt, accepted));
        }
    }
    Err(GenError::TooManyAttempts {
        attempts: cfg.max_attempts,
    })
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
    fn attempt_loop_counts_rejections() {
        let cfg = GenConfig::default();
        let solved = State::solved(P).unwrap();
        let unsolvable =
            State::from_tubes(P, &[&[0, 2, 1][..], &[0, 1, 1], &[0, 2, 2], &[]]).unwrap();
        let one_move = State::from_tubes(P, &[&[0, 0][..], &[1, 1, 1], &[2, 2, 2], &[0]]).unwrap();
        let mut queue = vec![one_move, unsolvable, solved, solved];
        let mut counts = RejectionCounts::default();
        let (state, attempt, accepted) =
            attempt_loop(&cfg, &mut counts, || queue.pop().unwrap()).unwrap();
        assert_eq!((state, attempt, accepted.opt_moves), (one_move, 4, 1));
        assert_eq!(
            counts,
            RejectionCounts {
                already_solved: 2,
                unsolvable: 1,
                ..RejectionCounts::default()
            }
        );
        assert_eq!(counts.total(), 3);
        let short = GenConfig {
            max_attempts: 2,
            ..cfg
        };
        assert_eq!(
            attempt_loop(&short, &mut (), || solved),
            Err(GenError::TooManyAttempts { attempts: 2 })
        );
        let e = evaluate_counted(&unsolvable, &cfg);
        assert!(e.states_expanded > 0);
        assert_eq!(evaluate_counted(&solved, &cfg).states_expanded, 0);
    }

    #[test]
    fn config_serializes() {
        let cfg = GenConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<GenConfig>(&json).unwrap(), cfg);
        assert_eq!(cfg.solver_limits().max_time, None);
    }
}
