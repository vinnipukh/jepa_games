//! The Python classes (`Params`, `GenConfig`, `State`, `Puzzle`, `SolveResult`) and the scalar
//! game functions.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use numpy::ndarray::Array2;
use numpy::{AllowTypeChange, IntoPyArray, PyArray1, PyArray2, PyArrayLike2};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use water_sort_core as core;
use water_sort_core::{EMPTY, Layout, Move, puzzle_code};

use crate::errors;

/// Puzzle shape: `n_colors` colors of `capacity` units each, in `n_colors + n_empty` tubes.
#[pyclass(
    name = "Params",
    module = "jepa_water_sort",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyParams(pub core::Params);

#[pymethods]
impl PyParams {
    #[new]
    #[pyo3(signature = (n_colors, capacity = core::Params::DEFAULT_CAPACITY, n_empty = core::Params::DEFAULT_N_EMPTY))]
    fn new(n_colors: u8, capacity: u8, n_empty: u8) -> PyResult<Self> {
        let params = core::Params {
            n_colors,
            capacity,
            n_empty,
        };
        params
            .validate()
            .map_err(|e| errors::value(format!("invalid params: {e}")))?;
        Ok(Self(params))
    }

    #[getter]
    const fn n_colors(&self) -> u8 {
        self.0.n_colors
    }

    #[getter]
    const fn capacity(&self) -> u8 {
        self.0.capacity
    }

    #[getter]
    const fn n_empty(&self) -> u8 {
        self.0.n_empty
    }

    #[getter]
    const fn n_tubes(&self) -> usize {
        self.0.n_tubes()
    }

    #[getter]
    const fn n_units(&self) -> usize {
        self.0.n_units()
    }

    /// Whether the configuration is in the supported range of `layout` (D3).
    #[pyo3(signature = (layout = "standard"))]
    fn is_supported(&self, layout: &str) -> PyResult<bool> {
        Ok(core::is_supported_in(&self.0, parse_layout(layout)?))
    }

    fn as_tuple(&self) -> (u8, u8, u8) {
        (self.0.n_colors, self.0.capacity, self.0.n_empty)
    }

    fn __repr__(&self) -> String {
        format!(
            "Params(n_colors={}, capacity={}, n_empty={})",
            self.0.n_colors, self.0.capacity, self.0.n_empty
        )
    }

    fn __getnewargs__(&self) -> (u8, u8, u8) {
        self.as_tuple()
    }
}

/// `Params` or an `(n_colors, capacity, n_empty)` tuple.
#[derive(FromPyObject)]
pub enum ParamsArg {
    Obj(PyParams),
    Tuple((u8, u8, u8)),
}

impl ParamsArg {
    pub fn get(&self) -> PyResult<core::Params> {
        match *self {
            Self::Obj(p) => Ok(p.0),
            Self::Tuple((n_colors, capacity, n_empty)) => {
                PyParams::new(n_colors, capacity, n_empty).map(|p| p.0)
            }
        }
    }
}

pub fn parse_layout(layout: &str) -> PyResult<Layout> {
    layout
        .parse()
        .map_err(|e: core::UnknownLayout| errors::value(e.to_string()))
}

/// Generation settings (`water_sort_core::GenConfig`). Part of a puzzle's reproducibility key.
#[pyclass(
    name = "GenConfig",
    module = "jepa_water_sort",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyGenConfig(pub core::GenConfig);

#[pymethods]
impl PyGenConfig {
    #[new]
    #[pyo3(signature = (
        min_opt = 1,
        max_opt = None,
        max_attempts = 10_000,
        max_states = core::SolverLimits::DEFAULT_MAX_STATES,
        random_rollouts = 64,
        rollout_cap_factor = 4,
        dead_end_ratios = false,
    ))]
    const fn new(
        min_opt: u32,
        max_opt: Option<u32>,
        max_attempts: u32,
        max_states: u64,
        random_rollouts: u32,
        rollout_cap_factor: u32,
        dead_end_ratios: bool,
    ) -> Self {
        Self(core::GenConfig {
            min_opt,
            max_opt,
            max_attempts,
            max_states,
            metrics: core::MetricsConfig {
                random_rollouts,
                rollout_cap_factor,
                dead_end_ratios,
            },
        })
    }

    #[getter]
    const fn min_opt(&self) -> u32 {
        self.0.min_opt
    }
    #[getter]
    const fn max_opt(&self) -> Option<u32> {
        self.0.max_opt
    }
    #[getter]
    const fn max_attempts(&self) -> u32 {
        self.0.max_attempts
    }
    #[getter]
    const fn max_states(&self) -> u64 {
        self.0.max_states
    }
    #[getter]
    const fn random_rollouts(&self) -> u32 {
        self.0.metrics.random_rollouts
    }
    #[getter]
    const fn rollout_cap_factor(&self) -> u32 {
        self.0.metrics.rollout_cap_factor
    }
    #[getter]
    const fn dead_end_ratios(&self) -> bool {
        self.0.metrics.dead_end_ratios
    }

    /// The JSON form used in dataset records and manifests (`gen_config_json`).
    fn to_json(&self) -> String {
        serde_json::to_string(&self.0).expect("GenConfig serializes")
    }

    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        serde_json::from_str(text)
            .map(Self)
            .map_err(|e| errors::value(format!("invalid GenConfig JSON: {e}")))
    }

    fn __repr__(&self) -> String {
        format!("GenConfig.from_json({:?})", self.to_json())
    }

    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        Ok((slf.getattr("from_json")?, (slf.get().to_json(),)))
    }
}

pub fn config_or_default(config: Option<PyGenConfig>) -> core::GenConfig {
    config.map_or_else(core::GenConfig::default, |c| c.0)
}

/// A puzzle position. Immutable; moves return a new state.
#[pyclass(name = "State", module = "jepa_water_sort", frozen, eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyState(pub core::State);

/// Builds a state from `n_tubes × capacity` cells (bottom to top, [`EMPTY`] above the fill).
/// `n_colors` follows from the number of units.
pub fn state_from_cells(
    cells: &[u8],
    n_tubes: usize,
    capacity: usize,
) -> Result<core::State, String> {
    if capacity == 0 || n_tubes == 0 || cells.len() != n_tubes * capacity {
        return Err(format!(
            "expected {n_tubes} x {capacity} cells, got {}",
            cells.len()
        ));
    }
    let mut tubes = Vec::with_capacity(n_tubes);
    let mut units = 0;
    for (i, row) in cells.chunks(capacity).enumerate() {
        let h = row.iter().take_while(|&&c| c != EMPTY).count();
        if row[h..].iter().any(|&c| c != EMPTY) {
            return Err(format!("tube {i} has a unit above an empty cell"));
        }
        units += h;
        tubes.push(&row[..h]);
    }
    if !units.is_multiple_of(capacity) {
        return Err(format!(
            "{units} units is not a multiple of the capacity {capacity}"
        ));
    }
    let params = core::Params {
        n_colors: u8::try_from(units / capacity).map_err(|_| "too many units".to_owned())?,
        capacity: u8::try_from(capacity).map_err(|_| "capacity too large".to_owned())?,
        n_empty: u8::try_from(n_tubes - units / capacity)
            .map_err(|_| "too many tubes".to_owned())?,
    };
    core::State::from_tubes(params, &tubes).map_err(|e| e.to_string())
}

/// Writes a state's padded cells, row-major (tube, bottom → top), into `out`.
pub fn write_cells(s: &core::State, out: &mut [u8]) {
    let cap = usize::from(s.capacity());
    for (i, row) in out.chunks_mut(cap).enumerate() {
        row.copy_from_slice(s.padded_tube(i));
    }
}

#[pymethods]
impl PyState {
    /// From tube contents, bottom to top. `n_colors` follows from the number of units.
    #[staticmethod]
    fn from_tubes(tubes: Vec<Vec<u8>>, capacity: u8) -> PyResult<Self> {
        let units: usize = tubes.iter().map(Vec::len).sum();
        let cap = usize::from(capacity).max(1);
        if !units.is_multiple_of(cap) {
            return Err(errors::value(format!(
                "{units} units is not a multiple of the capacity {capacity}"
            )));
        }
        let n_colors = u8::try_from(units / cap).map_err(|_| errors::value("too many units"))?;
        let n_empty = u8::try_from(tubes.len().saturating_sub(units / cap))
            .map_err(|_| errors::value("too many tubes"))?;
        let params = core::Params {
            n_colors,
            capacity,
            n_empty,
        };
        core::State::from_tubes(params, &tubes)
            .map(Self)
            .map_err(errors::state)
    }

    /// The solved standard layout of `params`.
    #[staticmethod]
    fn solved(params: ParamsArg) -> PyResult<Self> {
        core::State::solved(params.get()?)
            .map(Self)
            .map_err(errors::state)
    }

    /// Decodes a puzzle code.
    #[staticmethod]
    fn from_code(code: &str) -> PyResult<Self> {
        puzzle_code::decode(code)
            .map(Self)
            .map_err(errors::puzzle_code)
    }

    /// From a `(n_tubes, capacity)` uint8 array, bottom to top, `EMPTY` (255) above the fill
    /// height: the dataset `state` column reshaped.
    #[staticmethod]
    fn from_numpy(array: PyArrayLike2<'_, u8, AllowTypeChange>) -> PyResult<Self> {
        let view = array.as_array();
        let (n_tubes, capacity) = view.dim();
        let cells: Vec<u8> = view.iter().copied().collect();
        state_from_cells(&cells, n_tubes, capacity)
            .map(Self)
            .map_err(errors::value)
    }

    /// `(n_tubes, capacity)` uint8 array, bottom to top, `EMPTY` (255) above the fill height.
    fn to_numpy<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<u8>> {
        let s = &self.0;
        let mut cells = vec![0u8; s.n_tubes() * usize::from(s.capacity())];
        write_cells(s, &mut cells);
        Array2::from_shape_vec((s.n_tubes(), usize::from(s.capacity())), cells)
            .expect("shape matches")
            .into_pyarray(py)
    }

    /// The flat cells as `bytes` (the dataset `state` column).
    fn to_bytes(&self) -> Vec<u8> {
        let s = &self.0;
        let mut cells = vec![0u8; s.n_tubes() * usize::from(s.capacity())];
        write_cells(s, &mut cells);
        cells
    }

    #[getter]
    const fn params(&self) -> PyParams {
        PyParams(self.0.params())
    }

    #[getter]
    const fn n_tubes(&self) -> usize {
        self.0.n_tubes()
    }

    #[getter]
    const fn capacity(&self) -> u8 {
        self.0.capacity()
    }

    /// Tube contents as lists of colors, bottom to top.
    #[getter]
    fn tubes(&self) -> Vec<Vec<u32>> {
        // `Vec<u8>` would convert to `bytes`.
        (0..self.0.n_tubes())
            .map(|i| self.0.tube(i).iter().map(|&c| u32::from(c)).collect())
            .collect()
    }

    #[getter]
    fn puzzle_code(&self) -> String {
        puzzle_code::encode(&self.0)
    }

    fn is_solved(&self) -> bool {
        self.0.is_solved()
    }

    /// Vertically adjacent cells with different colors (the shaping potential is its negative).
    fn color_changes(&self) -> u32 {
        self.0.color_changes()
    }

    /// Maximal same-color runs over all tubes.
    fn segments(&self) -> u32 {
        self.0.segments()
    }

    /// The admissible solver heuristic `segments - n_colors`.
    fn heuristic(&self) -> u32 {
        core::heuristic(&self.0)
    }

    /// Whether the state has `layout`'s fill heights.
    fn layout_matches(&self, layout: &str) -> PyResult<bool> {
        Ok(self.0.layout_matches(parse_layout(layout)?))
    }

    fn __hash__(&self) -> u64 {
        // Only for Python dicts and sets; never persisted (persist `canonical_hash`).
        let mut h = DefaultHasher::new();
        self.0.hash(&mut h);
        h.finish()
    }

    fn __repr__(&self) -> String {
        format!("State({:?})", self.puzzle_code())
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }

    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        Ok((slf.getattr("from_code")?, (slf.get().puzzle_code(),)))
    }
}

/// A pour as a `(from, to)` tuple or an action index `from * n_tubes + to`.
#[derive(FromPyObject)]
pub enum MoveArg {
    Pair((u8, u8)),
    Index(usize),
}

impl MoveArg {
    pub fn get(&self, n_tubes: usize) -> PyResult<Move> {
        match *self {
            Self::Pair((from, to)) => Ok(Move::new(from, to)),
            Self::Index(i) => Move::from_action_index(i, n_tubes).ok_or_else(|| {
                errors::value(format!(
                    "action {i} is out of range for {n_tubes} tubes (n_tubes^2 = {})",
                    n_tubes * n_tubes
                ))
            }),
        }
    }
}

fn move_tuple(m: Move) -> (u8, u8) {
    (m.from, m.to)
}

/// Applies a pour: `(next_state, units_moved)`. Raises `ValueError` on an illegal move.
#[pyfunction]
pub fn step(state: PyState, r#move: MoveArg) -> PyResult<(PyState, u8)> {
    let m = r#move.get(state.0.n_tubes())?;
    core::apply(&state.0, m)
        .map(|(next, k)| (PyState(next), k))
        .map_err(|e| errors::illegal_move(m, e))
}

/// All legal pours as `(from, to)` tuples, in lexicographic order.
#[pyfunction]
pub fn legal_moves(state: PyState) -> Vec<(u8, u8)> {
    core::legal_moves(&state.0).map(move_tuple).collect()
}

/// Legality of every action `from * n_tubes + to`, as a bool array of length `n_tubes²`.
#[pyfunction]
pub fn action_mask(py: Python<'_>, state: PyState) -> Bound<'_, PyArray1<bool>> {
    core::action_mask(&state.0).into_pyarray(py)
}

/// Result of `solve`.
#[pyclass(
    name = "SolveResult",
    module = "jepa_water_sort",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq)]
pub struct PySolveResult(pub core::SolveResult);

#[pymethods]
impl PySolveResult {
    /// `"solvable"`, `"unsolvable"` or `"timeout"`.
    #[getter]
    const fn status(&self) -> &'static str {
        match self.0 {
            core::SolveResult::Solvable { .. } => "solvable",
            core::SolveResult::Unsolvable { .. } => "unsolvable",
            core::SolveResult::Timeout { .. } => "timeout",
        }
    }

    #[getter]
    const fn solvable(&self) -> bool {
        matches!(self.0, core::SolveResult::Solvable { .. })
    }

    #[getter]
    const fn opt_moves(&self) -> Option<u32> {
        self.0.opt_moves()
    }

    /// The optimal solution as `(from, to)` tuples, or `None` if not solvable.
    #[getter]
    fn solution(&self) -> Option<Vec<(u8, u8)>> {
        match &self.0 {
            core::SolveResult::Solvable { solution, .. } => {
                Some(solution.iter().copied().map(move_tuple).collect())
            }
            _ => None,
        }
    }

    #[getter]
    const fn states_expanded(&self) -> u64 {
        self.0.states_expanded()
    }

    fn __repr__(&self) -> String {
        let opt = self
            .opt_moves()
            .map_or_else(|| "None".to_owned(), |o| o.to_string());
        format!(
            "SolveResult(status={:?}, opt_moves={opt}, states_expanded={})",
            self.status(),
            self.states_expanded()
        )
    }
}

/// Solves optimally (A*). `max_time` (seconds) makes the result machine-dependent; generation
/// never uses it (D11). Releases the GIL.
#[pyfunction]
#[pyo3(signature = (state, max_states = core::SolverLimits::DEFAULT_MAX_STATES, max_time = None))]
pub fn solve(
    py: Python<'_>,
    state: PyState,
    max_states: u64,
    max_time: Option<f64>,
) -> PyResult<PySolveResult> {
    let max_time = max_time
        .map(|t| Duration::try_from_secs_f64(t).map_err(|e| errors::value(e.to_string())))
        .transpose()?;
    let limits = core::SolverLimits {
        max_states,
        max_time,
    };
    Ok(PySolveResult(py.detach(|| core::solve(&state.0, &limits))))
}

/// Stars (1–5) for solving a puzzle with optimum `opt` in `player_moves` pours (D5).
/// `config` is `(c4, c3, c2)` in per mille. Raises `StarBelowOptimalError` below the optimum.
#[pyfunction]
#[pyo3(signature = (player_moves, opt, config = None))]
pub fn stars(player_moves: u32, opt: u32, config: Option<(u32, u32, u32)>) -> PyResult<u8> {
    let cfg = config.map_or_else(core::StarConfig::default, |(c4, c3, c2)| core::StarConfig {
        c4_permille: c4,
        c3_permille: c3,
        c2_permille: c2,
    });
    core::stars(player_moves, opt, &cfg).map_err(errors::stars)
}

/// The stable canonical hash (xxh3-64 of the exact canonical form, D8).
#[pyfunction]
pub fn canonical_hash(state: PyState) -> u64 {
    core::canonical_hash(&state.0)
}

/// The exact canonical form under tube order and color relabeling (D8).
#[pyfunction]
pub fn canonical(state: PyState) -> PyState {
    PyState(core::canonical_full(&state.0))
}

/// The difficulty tier (`"easy"`, `"medium"`, `"hard"`) of a puzzle with `opt_moves`, or
/// `None` outside the supported range (D16).
#[pyfunction]
#[pyo3(signature = (params, opt_moves, layout = "standard"))]
pub fn tier(params: ParamsArg, opt_moves: u32, layout: &str) -> PyResult<Option<String>> {
    Ok(core::Tier::of(&params.get()?, parse_layout(layout)?, opt_moves).map(|t| t.to_string()))
}

/// `SplitMix64` (the dataset seed rule is `seed_i = splitmix64(master_seed ^ i)`).
#[pyfunction]
pub const fn splitmix64(x: u64) -> u64 {
    core::splitmix64(x)
}

/// A generated puzzle and everything recorded about it (the Phase 4 record fields).
#[pyclass(
    name = "Puzzle",
    module = "jepa_water_sort",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPuzzle {
    pub state: core::State,
    pub seed: u64,
    pub generator_id: String,
    pub generator_version: u32,
    pub generator_variant: String,
    pub layout: Layout,
    pub opt_moves: u32,
    pub solution: Vec<Move>,
    pub metrics: core::DifficultyMetrics,
    pub canonical_hash: u64,
    pub attempts: u32,
}

impl PyPuzzle {
    pub fn from_generated(p: core::GeneratedPuzzle, layout: Layout) -> Self {
        Self {
            state: p.state,
            seed: p.seed,
            generator_id: p.generator_id.to_owned(),
            generator_version: p.generator_version,
            generator_variant: p.generator_variant,
            layout,
            opt_moves: p.opt_moves,
            solution: p.solution,
            metrics: p.metrics,
            canonical_hash: p.canonical_hash,
            attempts: p.attempts,
        }
    }
}

#[pymethods]
impl PyPuzzle {
    #[getter]
    const fn state(&self) -> PyState {
        PyState(self.state)
    }
    #[getter]
    const fn params(&self) -> PyParams {
        PyParams(self.state.params())
    }
    #[getter]
    const fn seed(&self) -> u64 {
        self.seed
    }
    #[getter]
    fn generator_id(&self) -> &str {
        &self.generator_id
    }
    #[getter]
    const fn generator_version(&self) -> u32 {
        self.generator_version
    }
    #[getter]
    fn generator_variant(&self) -> &str {
        &self.generator_variant
    }
    #[getter]
    const fn layout(&self) -> &'static str {
        self.layout.name()
    }
    #[getter]
    const fn opt_moves(&self) -> u32 {
        self.opt_moves
    }
    /// The optimal solution as `(from, to)` tuples.
    #[getter]
    fn solution(&self) -> Vec<(u8, u8)> {
        self.solution.iter().copied().map(move_tuple).collect()
    }
    /// The optimal solution as action indices (the dataset `solution` column).
    #[getter]
    fn solution_actions(&self) -> Vec<usize> {
        let n = self.state.n_tubes();
        self.solution.iter().map(|m| m.action_index(n)).collect()
    }
    #[getter]
    const fn canonical_hash(&self) -> u64 {
        self.canonical_hash
    }
    #[getter]
    fn puzzle_code(&self) -> String {
        puzzle_code::encode(&self.state)
    }
    #[getter]
    const fn attempts(&self) -> u32 {
        self.attempts
    }
    /// `"easy"`, `"medium"`, `"hard"` (D16), or `None` outside the supported range.
    #[getter]
    fn tier(&self) -> Option<String> {
        core::Tier::of(&self.state.params(), self.layout, self.opt_moves).map(|t| t.to_string())
    }
    /// The difficulty metrics as a dict (the dataset `metrics_*` columns without the prefix).
    #[getter]
    fn metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let m = &self.metrics;
        let d = PyDict::new(py);
        d.set_item("opt_moves", m.opt_moves)?;
        d.set_item("states_expanded", m.states_expanded)?;
        d.set_item("color_changes", m.color_changes)?;
        d.set_item("segments", m.segments)?;
        d.set_item("random_rollouts", m.random_rollouts)?;
        d.set_item("random_stuck_rate", m.random_stuck_rate)?;
        d.set_item("random_capped_rate", m.random_capped_rate)?;
        d.set_item("dead_end_ratio_d1", m.dead_end_ratio_d1)?;
        d.set_item("dead_end_ratio_d2", m.dead_end_ratio_d2)?;
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!(
            "Puzzle(generator={:?}, variant={:?}, seed=0x{:016x}, code={:?}, opt_moves={})",
            self.generator_id,
            self.generator_variant,
            self.seed,
            self.puzzle_code(),
            self.opt_moves
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.state == other.state
            && self.seed == other.seed
            && self.generator_id == other.generator_id
            && self.generator_version == other.generator_version
            && self.generator_variant == other.generator_variant
            && self.layout == other.layout
            && self.opt_moves == other.opt_moves
            && self.solution == other.solution
            && self.metrics == other.metrics
            && self.canonical_hash == other.canonical_hash
            && self.attempts == other.attempts
    }

    /// Pickles through the puzzle code plus the recorded fields.
    fn __reduce__<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>)> {
        let py = slf.py();
        let p = slf.get();
        let m = &p.metrics;
        let metrics = (
            m.states_expanded,
            m.color_changes,
            m.segments,
            m.random_rollouts,
            m.random_stuck_rate,
            m.random_capped_rate,
            m.dead_end_ratio_d1,
            m.dead_end_ratio_d2,
        );
        let args = (
            p.puzzle_code(),
            p.seed,
            p.generator_id.clone(),
            p.generator_version,
            p.generator_variant.clone(),
            p.layout.name(),
            p.opt_moves,
            p.solution_actions(),
            metrics,
            p.canonical_hash,
            p.attempts,
        )
            .into_pyobject(py)?;
        Ok((py.get_type::<Self>().getattr("_restore")?, args))
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn _restore(
        code: &str,
        seed: u64,
        generator_id: String,
        generator_version: u32,
        generator_variant: String,
        layout: &str,
        opt_moves: u32,
        solution: Vec<usize>,
        metrics: (u64, u32, u32, u32, f32, f32, Option<f32>, Option<f32>),
        canonical_hash: u64,
        attempts: u32,
    ) -> PyResult<Self> {
        let state = puzzle_code::decode(code).map_err(errors::puzzle_code)?;
        let n = state.n_tubes();
        let solution = solution
            .into_iter()
            .map(|i| Move::from_action_index(i, n).ok_or_else(|| errors::value("bad action")))
            .collect::<PyResult<_>>()?;
        let (states_expanded, color_changes, segments, random_rollouts, stuck, capped, d1, d2) =
            metrics;
        Ok(Self {
            state,
            seed,
            generator_id,
            generator_version,
            generator_variant,
            layout: parse_layout(layout)?,
            opt_moves,
            solution,
            metrics: core::DifficultyMetrics {
                opt_moves,
                states_expanded,
                color_changes,
                segments,
                random_rollouts,
                random_stuck_rate: stuck,
                random_capped_rate: capped,
                dead_end_ratio_d1: d1,
                dead_end_ratio_d2: d2,
            },
            canonical_hash,
            attempts,
        })
    }
}
