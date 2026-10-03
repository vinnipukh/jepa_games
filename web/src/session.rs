//! `Session`: one player's attempt at a puzzle, on top of core's `Session` (Phase 1.5 counting:
//! undo does not decrement the counter, restart keeps it).

use wasm_bindgen::prelude::*;
use water_sort_core::{self as core, Move, StarConfig, State};

use crate::puzzle::Puzzle;

/// `n_tubes × capacity` cells, tube by tube, bottom to top, 255 above the fill (the dataset
/// `state` column).
pub(crate) fn cells(s: &State) -> Vec<u8> {
    (0..s.n_tubes())
        .flat_map(|i| s.padded_tube(i).iter().copied())
        .collect()
}

/// A play session. All rule checks and move counting are core's.
#[wasm_bindgen]
pub struct Session {
    pub(crate) puzzle: Puzzle,
    pub(crate) inner: core::Session,
}

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(puzzle: &Puzzle) -> Self {
        Self {
            puzzle: puzzle.clone(),
            inner: core::Session::new(*puzzle.state()),
        }
    }

    /// Pours `from` into `to`. Returns the number of units moved, or 0 for an illegal pour,
    /// which is not counted and changes nothing.
    pub fn pour(&mut self, from: u8, to: u8) -> u8 {
        self.inner.play(Move::new(from, to)).unwrap_or(0)
    }

    /// Why pouring `from` into `to` is illegal, or `None` if it is legal.
    pub fn why_illegal(&self, from: u8, to: u8) -> Option<String> {
        core::check(self.inner.state(), Move::new(from, to))
            .err()
            .map(|e| e.to_string())
    }

    /// Reverts the last pour since the start or the last restart. The counter is not
    /// decremented. Returns `false` if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    /// Back to the initial state; the counter is kept and the undo history cleared.
    pub fn restart(&mut self) {
        self.inner.restart();
    }

    /// Total pours on this puzzle, including undone ones and those before a restart.
    pub fn moves_counted(&self) -> u32 {
        self.inner.moves_made()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn is_solved(&self) -> bool {
        self.inner.is_solved()
    }

    /// Stars (1–5) once solved, `undefined` before or when the optimum is unknown.
    ///
    /// # Errors
    ///
    /// Fewer moves than the optimum (a solver bug).
    pub fn stars(&self) -> Result<Option<u8>, JsError> {
        let Some(opt) = self.puzzle.data.opt_moves else {
            return Ok(None);
        };
        Ok(self.inner.stars(opt, &StarConfig::default()).transpose()?)
    }

    /// Current cells: `n_tubes × capacity`, tube by tube, bottom to top, 255 = empty.
    pub fn cells(&self) -> Vec<u8> {
        cells(self.inner.state())
    }

    /// Legal pours as action indices (`from * n_tubes + to`).
    pub fn legal_moves(&self) -> Vec<u16> {
        let s = self.inner.state();
        core::legal_moves(s)
            .map(|m| u16::try_from(m.action_index(s.n_tubes())).expect("n_tubes <= 16"))
            .collect()
    }

    /// Puzzle code of the current state.
    pub fn state_code(&self) -> String {
        core::puzzle_code::encode(self.inner.state())
    }

    #[wasm_bindgen(getter)]
    pub fn n_tubes(&self) -> usize {
        self.inner.state().n_tubes()
    }

    #[wasm_bindgen(getter)]
    pub fn capacity(&self) -> u8 {
        self.inner.state().capacity()
    }

    #[wasm_bindgen(getter)]
    pub fn n_colors(&self) -> u8 {
        self.inner.state().params().n_colors
    }
}
