//! `Puzzle`: a generated puzzle, or a puzzle opened from its code, with its optimal solution.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use water_sort_core::{
    GenConfig, GeneratedPuzzle, Layout, Move, SolveResult, SolverLimits, State, Tier,
    canonical_hash, puzzle_code, replay, solve,
};

use crate::{JsParams, WEB_MAX_STATES, hex};

/// Everything the UI and the trajectory export know about a puzzle. Generator fields are `None`
/// for a puzzle opened from its code; `opt_moves` / `solution` are `None` when the solver gave
/// up or the start is unsolvable (`solver_status`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PuzzleData {
    pub generator_id: Option<String>,
    pub generator_version: Option<u32>,
    pub variant: Option<String>,
    pub layout: Layout,
    /// Hex.
    pub seed: Option<String>,
    /// The `GenConfig` the puzzle was generated with.
    pub config: Option<GenConfig>,
    pub attempts: Option<u32>,
    pub puzzle_code: String,
    /// Hex.
    pub canonical_hash: String,
    /// `"solvable"`, `"unsolvable"` or `"timeout"`.
    pub solver_status: String,
    pub opt_moves: Option<u32>,
    pub solution: Option<Vec<Move>>,
    pub tier: Option<Tier>,
}

/// A puzzle and its optimal solution.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct Puzzle {
    pub(crate) data: PuzzleData,
    pub(crate) state: State,
}

impl Puzzle {
    pub(crate) fn from_generated(g: GeneratedPuzzle, layout: Layout, cfg: &GenConfig) -> Self {
        let params = g.state.params();
        Self {
            data: PuzzleData {
                generator_id: Some(g.generator_id.into()),
                generator_version: Some(g.generator_version),
                variant: Some(g.generator_variant),
                layout,
                seed: Some(hex(g.seed)),
                config: Some(*cfg),
                attempts: Some(g.attempts),
                puzzle_code: g.puzzle_code,
                canonical_hash: hex(g.canonical_hash),
                solver_status: "solvable".into(),
                opt_moves: Some(g.opt_moves),
                solution: Some(g.solution),
                tier: Tier::of(&params, layout, g.opt_moves),
            },
            state: g.state,
        }
    }

    pub const fn state(&self) -> &State {
        &self.state
    }

    pub fn solution_moves(&self) -> Option<&[Move]> {
        self.data.solution.as_deref()
    }
}

/// The layout a state belongs to: standard if it has standard heights, else distributed.
fn layout_of(s: &State) -> Layout {
    if s.layout_matches(Layout::Standard) {
        Layout::Standard
    } else {
        Layout::Distributed
    }
}

/// Opens a puzzle from its code and solves it with a state budget (default
/// [`WEB_MAX_STATES`]; never a time limit, D11).
///
/// # Errors
///
/// A malformed code.
#[wasm_bindgen]
pub fn from_code(code: &str, max_states: Option<u32>) -> Result<Puzzle, JsError> {
    let state = puzzle_code::decode(code)?;
    let limits = SolverLimits::states(u64::from(max_states.unwrap_or(WEB_MAX_STATES)));
    let layout = layout_of(&state);
    let (status, opt_moves, solution) = match solve(&state, &limits) {
        SolveResult::Solvable {
            opt_moves,
            solution,
            ..
        } => ("solvable", Some(opt_moves), Some(solution)),
        SolveResult::Unsolvable { .. } => ("unsolvable", None, None),
        SolveResult::Timeout { .. } => ("timeout", None, None),
    };
    Ok(Puzzle {
        data: PuzzleData {
            generator_id: None,
            generator_version: None,
            variant: None,
            layout,
            seed: None,
            config: None,
            attempts: None,
            // Re-encoded, so lower-case or dashed input reads back canonical.
            puzzle_code: puzzle_code::encode(&state),
            canonical_hash: hex(canonical_hash(&state)),
            solver_status: status.into(),
            opt_moves,
            solution,
            tier: opt_moves.and_then(|o| Tier::of(&state.params(), layout, o)),
        },
        state,
    })
}

#[wasm_bindgen]
impl Puzzle {
    /// The puzzle as JSON, to pass it from the generation worker to the page.
    #[wasm_bindgen]
    pub fn to_json(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.data)?)
    }

    /// Reads [`Puzzle::to_json`] back. The code must decode, the hash must match it and a stored
    /// solution must solve it.
    #[wasm_bindgen]
    pub fn from_json(json: &str) -> Result<Self, JsError> {
        let data: PuzzleData = serde_json::from_str(json)?;
        let state = puzzle_code::decode(&data.puzzle_code)?;
        if hex(canonical_hash(&state)) != data.canonical_hash {
            return Err(JsError::new(
                "canonical hash does not match the puzzle code",
            ));
        }
        if let Some(sol) = &data.solution
            && !replay(&state, sol).is_some_and(|s| s.is_solved())
        {
            return Err(JsError::new("stored solution does not solve the puzzle"));
        }
        Ok(Self { data, state })
    }

    #[wasm_bindgen(getter)]
    pub fn generator_id(&self) -> Option<String> {
        self.data.generator_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn generator_version(&self) -> Option<u32> {
        self.data.generator_version
    }

    #[wasm_bindgen(getter)]
    pub fn variant(&self) -> Option<String> {
        self.data.variant.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn layout(&self) -> String {
        self.data.layout.name().into()
    }

    #[wasm_bindgen(getter)]
    pub fn params(&self) -> JsParams {
        JsParams::from_core(self.state.params())
    }

    /// Hex, 16 digits.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> Option<String> {
        self.data.seed.clone()
    }

    /// The `GenConfig` used, as JSON.
    #[wasm_bindgen(getter)]
    pub fn config_json(&self) -> Option<String> {
        self.data
            .config
            .and_then(|c| serde_json::to_string(&c).ok())
    }

    #[wasm_bindgen(getter)]
    pub fn attempts(&self) -> Option<u32> {
        self.data.attempts
    }

    #[wasm_bindgen(getter)]
    pub fn puzzle_code(&self) -> String {
        self.data.puzzle_code.clone()
    }

    /// Hex, 16 digits.
    #[wasm_bindgen(getter)]
    pub fn canonical_hash(&self) -> String {
        self.data.canonical_hash.clone()
    }

    /// `<generator_id>:<canonical hash>` as in the trajectory logger (`unknown` for a code).
    #[wasm_bindgen(getter)]
    pub fn puzzle_id(&self) -> String {
        format!(
            "{}:{}",
            self.data.generator_id.as_deref().unwrap_or("unknown"),
            self.data.canonical_hash
        )
    }

    #[wasm_bindgen(getter)]
    pub fn solver_status(&self) -> String {
        self.data.solver_status.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn opt_moves(&self) -> Option<u32> {
        self.data.opt_moves
    }

    /// The optimal solution as action indices (`from * n_tubes + to`).
    #[wasm_bindgen(getter)]
    pub fn solution(&self) -> Option<Vec<u16>> {
        let n = self.state.n_tubes();
        self.data
            .solution
            .as_ref()
            .map(|sol| sol.iter().map(|&m| crate::action_u16(m, n)).collect())
    }

    #[wasm_bindgen(getter)]
    pub fn tier(&self) -> Option<String> {
        self.data.tier.map(|t| t.to_string())
    }

    #[wasm_bindgen(getter)]
    pub fn n_tubes(&self) -> usize {
        self.state.n_tubes()
    }

    /// The initial state's cells (see [`crate::Session::cells`]).
    #[wasm_bindgen(getter)]
    pub fn cells(&self) -> Vec<u8> {
        crate::session::cells(&self.state)
    }
}
