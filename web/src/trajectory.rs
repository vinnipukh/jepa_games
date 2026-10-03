//! Human trajectory export (Phase 5.3 row format, `source = "human"`), download only.
//!
//! Every pour, illegal pour, undo and restart of a [`crate::Session`] is a row. The JEPA data,
//! the effective `(state, action, next_state)` transitions, is derived on import
//! (`jepa_water_sort.logger.import_web_exports`), which replays every row through core again.

use serde::Serialize;
use water_sort_core::{GenConfig, Params, State, Tier};

use crate::puzzle::Puzzle;
use crate::session::cells;

/// Export format name and version; the Python importer checks both.
pub const FORMAT: &str = "jepa_water_sort.human_trajectory";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Pour,
    Illegal,
    Undo,
    Restart,
}

/// One recorded player action.
#[derive(Clone, Debug)]
pub struct Event {
    pub kind: EventKind,
    /// Milliseconds since the session started.
    pub t_ms: f64,
    /// `from * n_tubes + to` for pours and illegal pours.
    pub action: Option<u16>,
    pub state: State,
    pub next_state: State,
    pub units_moved: u8,
    /// The move counter after the event.
    pub moves_counted: u32,
}

#[derive(Serialize)]
struct PuzzleJson<'a> {
    puzzle_id: String,
    generator_id: Option<&'a str>,
    generator_version: Option<u32>,
    variant: Option<&'a str>,
    layout: &'a str,
    seed: Option<&'a str>,
    params: Params,
    config: Option<GenConfig>,
    puzzle_code: &'a str,
    canonical_hash: &'a str,
    opt_moves: Option<u32>,
    tier: Option<Tier>,
}

// The flags are the Phase 5.3 columns.
#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize)]
struct Row<'a> {
    // Phase 5.3 columns.
    puzzle_id: &'a str,
    generator_id: &'a str,
    canonical_hash: &'a str,
    record_id: Option<u64>,
    split: Option<&'a str>,
    source: &'static str,
    episode: u32,
    step: usize,
    state: Vec<u8>,
    action: Option<u16>,
    next_state: Vec<u8>,
    done: bool,
    truncated: bool,
    illegal: bool,
    solved: bool,
    // Human-play extras.
    event: EventKind,
    t_ms: f64,
    units_moved: u8,
    moves_counted: u32,
}

#[derive(Serialize)]
struct Export<'a> {
    format: &'static str,
    format_version: u32,
    source: &'static str,
    /// Random, identifies nothing but this play session.
    session_id: String,
    tool_version: String,
    /// Unix milliseconds.
    started_at_ms: f64,
    exported_at_ms: f64,
    state_encoding: &'static str,
    puzzle: PuzzleJson<'a>,
    moves_counted: u32,
    solved: bool,
    stars: Option<u8>,
    rows: Vec<Row<'a>>,
}

pub struct ExportInput<'a> {
    pub puzzle: &'a Puzzle,
    pub session_id: u64,
    pub started_at_ms: f64,
    pub exported_at_ms: f64,
    pub events: &'a [Event],
    pub moves_counted: u32,
    pub solved: bool,
    pub stars: Option<u8>,
}

/// The export JSON.
///
/// # Errors
///
/// Serialization failure (not expected).
pub fn export_json(input: &ExportInput<'_>) -> serde_json::Result<String> {
    let d = &input.puzzle.data;
    let puzzle_id = input.puzzle.puzzle_id();
    let generator_id = d.generator_id.as_deref().unwrap_or("unknown");
    let rows = input
        .events
        .iter()
        .enumerate()
        .map(|(step, e)| {
            let solved = e.next_state.is_solved();
            Row {
                puzzle_id: &puzzle_id,
                generator_id,
                canonical_hash: &d.canonical_hash,
                record_id: None,
                split: None,
                source: "human",
                episode: 0,
                step,
                state: cells(&e.state),
                action: e.action,
                next_state: cells(&e.next_state),
                done: solved,
                truncated: false,
                illegal: e.kind == EventKind::Illegal,
                solved,
                event: e.kind,
                t_ms: e.t_ms,
                units_moved: e.units_moved,
                moves_counted: e.moves_counted,
            }
        })
        .collect();
    let export = Export {
        format: FORMAT,
        format_version: FORMAT_VERSION,
        source: "human",
        session_id: crate::hex(input.session_id),
        tool_version: format!("water_sort_web {}", env!("CARGO_PKG_VERSION")),
        started_at_ms: input.started_at_ms,
        exported_at_ms: input.exported_at_ms,
        state_encoding: "row-major n_tubes x capacity, bottom to top, 255 = empty",
        puzzle: PuzzleJson {
            puzzle_id: puzzle_id.clone(),
            generator_id: d.generator_id.as_deref(),
            generator_version: d.generator_version,
            variant: d.variant.as_deref(),
            layout: d.layout.name(),
            seed: d.seed.as_deref(),
            params: input.puzzle.state().params(),
            config: d.config,
            puzzle_code: &d.puzzle_code,
            canonical_hash: &d.canonical_hash,
            opt_moves: d.opt_moves,
            tier: d.tier,
        },
        moves_counted: input.moves_counted,
        solved: input.solved,
        stars: input.stars,
        rows,
    };
    serde_json::to_string(&export)
}
