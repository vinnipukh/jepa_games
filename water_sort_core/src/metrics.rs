//! Difficulty metrics recorded with every generated puzzle.

use serde::{Deserialize, Serialize};

/// Difficulty measurements of one puzzle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DifficultyMetrics {
    pub opt_moves: u32,
    /// States the A* solver expanded.
    pub states_expanded: u64,
    pub color_changes: u32,
    pub segments: u32,
    pub random_rollouts: u32,
    /// Fraction of rollouts that reached a state with no legal moves.
    pub random_stuck_rate: f32,
    /// Fraction of rollouts that hit the step cap without solving or getting stuck.
    pub random_capped_rate: f32,
    /// Fraction of distinct depth-1 states that are unsolvable (opt-in).
    pub dead_end_ratio_d1: Option<f32>,
    /// Fraction of distinct depth-2 states that are unsolvable (opt-in).
    pub dead_end_ratio_d2: Option<f32>,
}
