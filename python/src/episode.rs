//! The environment step (`water_sort_core::episode`), for `WaterSortEnv`.

use pyo3::prelude::*;
use water_sort_core::{EpisodeRules, episode_step};

use crate::errors;
use crate::types::PyState;

/// `(next_state, units_moved, reward, terminated, truncated, illegal, dead_end, solved)`.
type EnvStep = (PyState, u8, f64, bool, bool, bool, bool, bool);

/// One environment step under the shared episode rules (`water_sort_core::episode`): an illegal
/// action keeps the state and is flagged, reward −1 plus optional potential-based shaping,
/// termination on solved or dead end, truncation at `move_limit` actions.
#[pyfunction]
#[pyo3(signature = (state, action, moves_so_far, move_limit, shaping_gamma = None, dead_end_max_states = None))]
pub fn env_step(
    py: Python<'_>,
    state: PyState,
    action: usize,
    moves_so_far: u32,
    move_limit: u32,
    shaping_gamma: Option<f64>,
    dead_end_max_states: Option<u64>,
) -> PyResult<EnvStep> {
    let rules = EpisodeRules {
        move_limit,
        shaping_gamma,
        dead_end_max_states,
    };
    let run = || episode_step(&state.0, action, moves_so_far, &rules);
    // Only a solver dead-end check is slow enough to be worth releasing the GIL.
    let out = if dead_end_max_states.is_some() {
        py.detach(run)
    } else {
        run()
    }
    .map_err(|e| errors::value(e.to_string()))?;
    Ok((
        PyState(out.state),
        out.units_moved,
        out.reward,
        out.terminated,
        out.truncated,
        out.illegal,
        out.dead_end,
        out.solved,
    ))
}

/// The move limit `k · opt_moves` (D4).
#[pyfunction]
pub const fn move_limit(k: u32, opt_moves: u32) -> u32 {
    water_sort_core::move_limit(k, opt_moves)
}

/// Whether `state` is a dead end: not solved and no legal move, or (with `max_states`) proven
/// unsolvable by the solver.
#[pyfunction]
#[pyo3(signature = (state, max_states = None))]
pub fn is_dead_end(py: Python<'_>, state: PyState, max_states: Option<u64>) -> bool {
    py.detach(|| water_sort_core::is_dead_end(&state.0, max_states))
}
