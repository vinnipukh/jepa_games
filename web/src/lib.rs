//! `water_sort_web`: the browser binding (wasm-bindgen) of `water_sort_core` and the two
//! generators.
//!
//! Thin wrappers only. Every rule (moves, move counting, solving, stars, hashing, generation)
//! runs in the Rust crates, so a seed or puzzle code gives the same puzzle as in Rust and Python.
//! The TypeScript app only renders and forwards clicks.
//!
//! 64-bit values (seeds, canonical hashes) cross the JS boundary as 16-digit hex strings, never
//! as JS numbers (rule 8). Core never reads the clock (D1): this crate reads `Date.now()` and
//! passes it to `fresh_seed`.

#![forbid(unsafe_code)]
// wasm-bindgen exports take owned `String`s / `Option`s and return `Result<_, JsError>`; JS class
// arguments such as `&JsParams` must be references (by value would consume the JS object).
#![allow(
    clippy::needless_pass_by_value,
    clippy::missing_errors_doc,
    clippy::trivially_copy_pass_by_ref
)]

mod generate;
mod puzzle;
mod session;
mod trajectory;

use serde::Serialize;
use wasm_bindgen::prelude::*;
use water_sort_core::{self as core, Layout, Move, StarConfig};

pub use generate::{
    WEB_MAX_STATES, fresh_seed, generate, generate_with_config, strategies, variant, web_config,
    web_config_json,
};
pub use puzzle::{Puzzle, from_code};
pub use session::Session;
pub use trajectory::{FORMAT as TRAJECTORY_FORMAT, FORMAT_VERSION as TRAJECTORY_FORMAT_VERSION};

/// Crate version, e.g. for the trajectory export's `tool_version`.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// The empty cell value in [`Session::cells`] (255, as in the dataset `state` column).
#[wasm_bindgen]
pub fn empty_cell() -> u8 {
    core::EMPTY
}

/// Puzzle shape: `n_colors` colors of `capacity` units, plus `n_empty` empty tubes' worth of
/// free space.
#[wasm_bindgen(js_name = Params)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsParams {
    pub n_colors: u8,
    pub capacity: u8,
    pub n_empty: u8,
}

#[wasm_bindgen(js_class = Params)]
impl JsParams {
    #[wasm_bindgen(constructor)]
    pub fn new(n_colors: u8, capacity: u8, n_empty: u8) -> Self {
        Self {
            n_colors,
            capacity,
            n_empty,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn n_tubes(&self) -> usize {
        self.core().n_tubes()
    }

    /// Whether these params are in the supported range of `layout` (D3 and its addendum).
    pub fn is_supported(&self, layout: &str) -> Result<bool, JsError> {
        Ok(core::is_supported_in(&self.core(), parse_layout(layout)?))
    }
}

impl JsParams {
    pub const fn core(self) -> core::Params {
        core::Params {
            n_colors: self.n_colors,
            capacity: self.capacity,
            n_empty: self.n_empty,
        }
    }

    pub const fn from_core(p: core::Params) -> Self {
        Self {
            n_colors: p.n_colors,
            capacity: p.capacity,
            n_empty: p.n_empty,
        }
    }
}

pub(crate) fn parse_layout(layout: &str) -> Result<Layout, JsError> {
    layout
        .parse()
        .map_err(|e: core::UnknownLayout| JsError::new(&e.to_string()))
}

/// Seeds are 16-digit hex strings; up to 16 hex digits, optionally prefixed with `0x`, are read.
pub(crate) fn parse_seed(seed: &str) -> Result<u64, JsError> {
    let s = seed.trim();
    let digits = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if digits.is_empty() || digits.len() > 16 {
        return Err(JsError::new(&format!(
            "seed {seed:?}: expected 1 to 16 hex digits"
        )));
    }
    u64::from_str_radix(digits, 16)
        .map_err(|_| JsError::new(&format!("seed {seed:?}: expected hex digits")))
}

/// `m` as an action index `from * n_tubes + to` (below 16 × 16, so it fits).
pub(crate) fn action_u16(m: Move, n_tubes: usize) -> u16 {
    u16::try_from(m.action_index(n_tubes)).unwrap_or(u16::MAX)
}

pub(crate) fn hex(x: u64) -> String {
    format!("{x:016x}")
}

#[derive(Serialize)]
struct SupportedRowJson {
    capacity: u8,
    n_empty: u8,
    min_colors: u8,
    max_colors: u8,
}

/// The supported range of `layout` as JSON: `[{capacity, n_empty, min_colors, max_colors}]`.
/// The UI only offers these params.
#[wasm_bindgen]
pub fn supported_rows(layout: &str) -> Result<String, JsError> {
    let rows: Vec<SupportedRowJson> = core::supported_rows(parse_layout(layout)?)
        .iter()
        .map(|r| SupportedRowJson {
            capacity: r.capacity,
            n_empty: r.n_empty,
            min_colors: r.min_colors,
            max_colors: r.max_colors,
        })
        .collect();
    Ok(serde_json::to_string(&rows)?)
}

/// Stars (1–5) for solving a puzzle with optimum `opt_moves` in `player_moves` pours (D5).
///
/// # Errors
///
/// `player_moves < opt_moves` (a solver bug).
#[wasm_bindgen]
pub fn stars(player_moves: u32, opt_moves: u32) -> Result<u8, JsError> {
    Ok(core::stars(
        player_moves,
        opt_moves,
        &StarConfig::default(),
    )?)
}

/// The most moves that still earn 5, 4, 3 and 2 stars: `[opt, opt + t4, opt + t3, opt + t2]`.
#[wasm_bindgen]
pub fn star_moves(opt_moves: u32) -> Vec<u32> {
    let (t4, t3, t2) = core::thresholds(opt_moves, &StarConfig::default());
    vec![
        opt_moves,
        opt_moves.saturating_add(t4),
        opt_moves.saturating_add(t3),
        opt_moves.saturating_add(t2),
    ]
}

/// `canonical_hash` (16 hex digits) of the state a puzzle code describes.
#[wasm_bindgen]
pub fn canonical_hash(code: &str) -> Result<String, JsError> {
    Ok(hex(core::canonical_hash(&core::puzzle_code::decode(code)?)))
}

/// `splitmix64` on hex strings (the dataset seed rule, D17).
#[wasm_bindgen]
pub fn splitmix64(x: &str) -> Result<String, JsError> {
    Ok(hex(core::splitmix64(parse_seed(x)?)))
}

/// Milliseconds since the Unix epoch: `Date.now()` on wasm32, the system clock natively.
pub(crate) fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}
