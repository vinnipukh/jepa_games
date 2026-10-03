//! `Session`: one player's attempt at a puzzle, on top of core's `Session` (Phase 1.5 counting:
//! undo does not decrement the counter, restart keeps it).

use wasm_bindgen::prelude::*;
use water_sort_core::{self as core, Move, StarConfig, State};

use crate::now_ms;
use crate::puzzle::Puzzle;
use crate::trajectory::{Event, EventKind, ExportInput, export_json};

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
    id: u64,
    started_at_ms: f64,
    /// Every pour, illegal pour, undo and restart, for the trajectory export.
    events: Vec<Event>,
}

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(puzzle: &Puzzle) -> Self {
        Self {
            puzzle: puzzle.clone(),
            inner: core::Session::new(*puzzle.state()),
            // A random id with no personal data; 0 if the entropy source fails.
            id: getrandom::u64().unwrap_or(0),
            started_at_ms: now_ms(),
            events: Vec::new(),
        }
    }

    /// Pours `from` into `to`. Returns the number of units moved, or 0 for an illegal pour,
    /// which is not counted and changes nothing.
    pub fn pour(&mut self, from: u8, to: u8) -> u8 {
        let before = *self.inner.state();
        let m = Move::new(from, to);
        let result = self.inner.play(m);
        let n = before.n_tubes();
        if usize::from(from) < n && usize::from(to) < n {
            let (kind, units) = match result {
                Ok(k) => (EventKind::Pour, k),
                Err(_) => (EventKind::Illegal, 0),
            };
            self.record(kind, Some(crate::action_u16(m, n)), before, units);
        }
        result.unwrap_or(0)
    }

    fn record(&mut self, kind: EventKind, action: Option<u16>, state: State, units_moved: u8) {
        self.events.push(Event {
            kind,
            t_ms: now_ms() - self.started_at_ms,
            action,
            state,
            next_state: *self.inner.state(),
            units_moved,
            moves_counted: self.inner.moves_made(),
        });
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
        let before = *self.inner.state();
        let undone = self.inner.undo();
        if undone {
            self.record(EventKind::Undo, None, before, 0);
        }
        undone
    }

    /// Back to the initial state; the counter is kept and the undo history cleared.
    pub fn restart(&mut self) {
        let before = *self.inner.state();
        self.inner.restart();
        self.record(EventKind::Restart, None, before, 0);
    }

    /// Number of recorded events (pours, illegal pours, undos, restarts).
    pub fn n_events(&self) -> usize {
        self.events.len()
    }

    /// The session as a trajectory export (JSON, Phase 5.3 rows, `source = "human"`).
    ///
    /// # Errors
    ///
    /// Fewer moves than the optimum (a solver bug).
    pub fn export_trajectory(&self) -> Result<String, JsError> {
        Ok(export_json(&ExportInput {
            puzzle: &self.puzzle,
            session_id: self.id,
            started_at_ms: self.started_at_ms,
            exported_at_ms: now_ms(),
            events: &self.events,
            moves_counted: self.inner.moves_made(),
            solved: self.inner.is_solved(),
            stars: self.stars()?,
        })?)
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
            .map(|m| crate::action_u16(m, s.n_tubes()))
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
