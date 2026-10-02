//! Water Sort game core: state, move rules, solver, star metric, canonical hash and the
//! `Generator` trait. Every other crate takes its game logic from here.

#![forbid(unsafe_code)]

pub mod canon;
pub mod generator;
pub mod metrics;
pub mod moves;
pub mod params;
pub mod sampling;
pub mod seed;
pub mod solver;
pub mod stars;
pub mod state;

pub use canon::{canonical_full, canonical_hash, canonical_tubes, solver_key};
pub use generator::{
    Accepted, GenConfig, GenError, GeneratedPuzzle, Generator, MetricsConfig, Rejection, evaluate,
};
pub use metrics::{DifficultyMetrics, compute_metrics};
pub use moves::{
    InvalidReverseMove, Move, MoveError, ReverseMove, action_mask, apply, check, is_legal,
    is_valid_reverse, legal_moves, reverse_moves, unapply,
};
pub use params::{MAX_CAP, MAX_TUBES, Params, ParamsError};
pub use sampling::{bounded_u32, fisher_yates};
pub use seed::{splitmix64, time_seed};
pub use solver::{SolveResult, SolverLimits, heuristic, replay, solve, solve_astar, solve_bfs};
pub use stars::{Session, StarConfig, StarError, stars, thresholds};
pub use state::{EMPTY, State, StateError};
