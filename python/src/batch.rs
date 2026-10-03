//! Batched functions. They copy their inputs out of Python, release the GIL and run on rayon.

use numpy::ndarray::{Array1, Array3};
use numpy::{AllowTypeChange, IntoPyArray, PyArray1, PyArray3, PyArrayLike1, PyArrayLike3};
use pyo3::prelude::*;
use rayon::prelude::*;
use water_sort_core::{Move, apply};

use crate::errors;
use crate::generate::AnyGenerator;
use crate::types::{
    ParamsArg, PyGenConfig, PyPuzzle, config_or_default, state_from_cells, write_cells,
};

/// Below this many rows the batch runs on the calling thread: rayon's overhead would dominate.
const PARALLEL_MIN: usize = 256;

/// `(next_states, units_moved)`.
type StepArrays<'py> = (Bound<'py, PyArray3<u8>>, Bound<'py, PyArray1<u8>>);

/// Applies `actions[i]` (index `from * n_tubes + to`) to `states[i]`.
///
/// `states` is a `(B, n_tubes, capacity)` uint8 array in the `State.to_numpy()` format. Returns
/// `(next_states, units_moved)`; an illegal action leaves the row unchanged and moves 0 units
/// (every legal pour moves at least one). Raises `ValueError` for a malformed state or an
/// action outside `0..n_tubes²`. Releases the GIL.
#[pyfunction]
pub fn batch_step<'py>(
    py: Python<'py>,
    states: PyArrayLike3<'py, u8, AllowTypeChange>,
    actions: PyArrayLike1<'py, i64, AllowTypeChange>,
) -> PyResult<StepArrays<'py>> {
    let (b, n_tubes, capacity) = states.as_array().dim();
    let n_actions = actions.as_array().len();
    if n_actions != b {
        return Err(errors::value(format!("{b} states but {n_actions} actions")));
    }
    let mut cells: Vec<u8> = states.as_array().iter().copied().collect();
    let actions: Vec<i64> = actions.as_array().iter().copied().collect();
    let row = n_tubes * capacity;
    let (cells, units) = py
        .detach(move || -> Result<_, String> {
            let mut units = vec![0u8; b];
            if row > 0 {
                let step_row =
                    |(i, (out, k)): (usize, (&mut [u8], &mut u8))| -> Result<(), String> {
                        let s = state_from_cells(out, n_tubes, capacity)
                            .map_err(|e| format!("states[{i}]: {e}"))?;
                        let a = usize::try_from(actions[i]).ok();
                        let m = a
                            .and_then(|a| Move::from_action_index(a, n_tubes))
                            .ok_or_else(|| {
                                format!(
                                    "actions[{i}] = {} is out of range 0..{}",
                                    actions[i],
                                    n_tubes * n_tubes
                                )
                            })?;
                        if let Ok((next, moved)) = apply(&s, m) {
                            write_cells(&next, out);
                            *k = moved;
                        }
                        Ok(())
                    };
                // Collected per row so the reported error is the first bad row, whatever the
                // scheduling.
                let results: Vec<Result<(), String>> = cells
                    .par_chunks_mut(row)
                    .zip(units.par_iter_mut())
                    .enumerate()
                    .with_min_len(PARALLEL_MIN)
                    .map(step_row)
                    .collect();
                results.into_iter().collect::<Result<(), String>>()?;
            }
            Ok((cells, units))
        })
        .map_err(errors::value)?;
    let next = Array3::from_shape_vec((b, n_tubes, capacity), cells).expect("shape matches");
    Ok((next.into_pyarray(py), Array1::from(units).into_pyarray(py)))
}

/// Generates one puzzle per seed (or `count` puzzles with fresh seeds), in parallel. The result
/// is in seed order and equals `[generate(..., seed=s) for s in seeds]`. Releases the GIL.
#[pyfunction]
#[pyo3(signature = (generator, params, seeds = None, count = None, config = None, strategy = None, layout = "standard"))]
#[allow(clippy::too_many_arguments)]
pub fn batch_generate(
    py: Python<'_>,
    generator: &str,
    params: ParamsArg,
    seeds: Option<Vec<u64>>,
    count: Option<usize>,
    config: Option<PyGenConfig>,
    strategy: Option<&str>,
    layout: &str,
) -> PyResult<Vec<PyPuzzle>> {
    let g = AnyGenerator::new(generator, layout, strategy)?;
    let params = params.get()?;
    let cfg = config_or_default(config);
    let seeds = match (seeds, count) {
        (Some(seeds), None) => seeds,
        (None, Some(count)) => (0..count)
            .map(|_| g.fresh_seed())
            .collect::<Result<_, _>>()
            .map_err(errors::generation)?,
        _ => return Err(errors::value("pass exactly one of seeds and count")),
    };
    py.detach(|| {
        seeds
            .par_iter()
            .map(|&seed| g.generate(&params, seed, &cfg))
            .collect::<Result<Vec<_>, _>>()
    })
    .map(|puzzles| {
        puzzles
            .into_iter()
            .map(|p| PyPuzzle::from_generated(p, g.layout()))
            .collect()
    })
    .map_err(errors::generation)
}
