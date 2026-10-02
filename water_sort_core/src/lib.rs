//! Water Sort game core: state, move rules, solver, star metric, canonical hash and the
//! `Generator` trait. Every other crate takes its game logic from here.

#![forbid(unsafe_code)]

pub mod canon;
pub mod enumerate;
pub mod generator;
pub mod layout;
pub mod metrics;
pub mod moves;
pub mod params;
pub mod puzzle_code;
pub mod sampling;
pub mod seed;
pub mod solver;
pub mod stars;
pub mod state;
pub mod supported;
pub mod symmetry;

pub use canon::{canonical_full, canonical_hash, canonical_tubes, solver_key};
pub use enumerate::{distributed_fills, standard_fills};
pub use generator::{
    Accepted, Evaluation, GenConfig, GenError, GeneratedPuzzle, Generator, MetricsConfig, Observer,
    Rejection, RejectionCounts, attempt_loop, evaluate, evaluate_counted, try_attempt_loop,
};
pub use layout::{HeightCounts, Layout, UnknownLayout, sample_heights};
pub use metrics::{DifficultyMetrics, compute_metrics};
pub use moves::{
    InvalidReverseMove, Move, MoveError, ReverseMove, action_mask, apply, check, has_reverse_move,
    is_legal, is_valid_reverse, legal_moves, reverse_moves, unapply,
};
pub use params::{MAX_CAP, MAX_TUBES, Params, ParamsError};
pub use puzzle_code::PuzzleCodeError;
pub use sampling::{bounded_u32, bounded_u64, fisher_yates};
pub use seed::{splitmix64, time_seed};
pub use solver::{SolveResult, SolverLimits, heuristic, replay, solve, solve_astar, solve_bfs};
pub use stars::{Session, StarConfig, StarError, stars, thresholds};
pub use state::{EMPTY, State, StateError};
pub use supported::{
    MAX_SUPPORTED_EMPTY, SUPPORTED, SUPPORTED_DISTRIBUTED, SupportedRow, is_supported,
    is_supported_in, supported_rows,
};
pub use symmetry::is_symmetric;
