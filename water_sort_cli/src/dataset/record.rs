//! The dataset record: one generated puzzle, as stored in JSONL and Parquet (Phase 4 schema).

use serde::{Deserialize, Serialize};
use water_sort_core::{
    DifficultyMetrics, EMPTY, GeneratedPuzzle, Layout, Move, Params, State, StateError, Tier,
};

use super::split::{Split, SplitRanges};

/// One puzzle of a dataset. Field order is the column order of both formats.
///
/// In JSON, `seed` and `canonical_hash` are 16-digit hex strings (JavaScript cannot read 64-bit
/// integers exactly) and `created_at` is RFC 3339; the metrics are flattened into
/// `metrics_*` fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Index within the run; record `i` was generated from `splitmix64(master_seed ^ i)`.
    pub record_id: u64,
    pub generator_id: String,
    pub generator_version: u32,
    pub generator_variant: String,
    pub layout: Layout,
    pub params: Params,
    /// `GenConfig` as JSON.
    pub gen_config: String,
    #[serde(with = "hex_u64")]
    pub seed: u64,
    pub puzzle_code: String,
    /// `n_tubes × capacity` cells, tube by tube, bottom to top, [`EMPTY`] (255) above the fill.
    pub state: Vec<u8>,
    pub opt_moves: u32,
    /// `None` outside the supported range (`--allow-unsupported`), where no tier table exists.
    pub tier: Option<Tier>,
    /// Action indices `from * n_tubes + to`.
    pub solution: Vec<u16>,
    #[serde(with = "hex_u64")]
    pub canonical_hash: u64,
    pub attempts: u32,
    #[serde(flatten)]
    pub metrics: RecordMetrics,
    pub split: Split,
    /// Unix nanoseconds, UTC.
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    /// Crate version + git commit of the tool that wrote the record.
    pub tool_version: String,
}

/// [`DifficultyMetrics`] without `opt_moves` (a column of its own), flattened into the record.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordMetrics {
    #[serde(rename = "metrics_states_expanded")]
    pub states_expanded: u64,
    #[serde(rename = "metrics_color_changes")]
    pub color_changes: u32,
    #[serde(rename = "metrics_segments")]
    pub segments: u32,
    #[serde(rename = "metrics_random_rollouts")]
    pub random_rollouts: u32,
    #[serde(rename = "metrics_random_stuck_rate")]
    pub random_stuck_rate: f32,
    #[serde(rename = "metrics_random_capped_rate")]
    pub random_capped_rate: f32,
    #[serde(rename = "metrics_dead_end_ratio_d1")]
    pub dead_end_ratio_d1: Option<f32>,
    #[serde(rename = "metrics_dead_end_ratio_d2")]
    pub dead_end_ratio_d2: Option<f32>,
}

impl From<&DifficultyMetrics> for RecordMetrics {
    fn from(m: &DifficultyMetrics) -> Self {
        Self {
            states_expanded: m.states_expanded,
            color_changes: m.color_changes,
            segments: m.segments,
            random_rollouts: m.random_rollouts,
            random_stuck_rate: m.random_stuck_rate,
            random_capped_rate: m.random_capped_rate,
            dead_end_ratio_d1: m.dead_end_ratio_d1,
            dead_end_ratio_d2: m.dead_end_ratio_d2,
        }
    }
}

/// The run-level fields every record of one dataset shares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunInfo {
    pub params: Params,
    pub layout: Layout,
    /// `GenConfig` as JSON.
    pub gen_config: String,
    pub split: SplitRanges,
    pub created_at: i64,
    pub tool_version: String,
}

/// Why a stored state is not a valid [`State`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateDecodeError {
    #[error("state has {got} cells, expected {expected}")]
    Length { expected: usize, got: usize },
    #[error("tube {0} has a unit above an empty cell")]
    Floating(usize),
    #[error(transparent)]
    State(#[from] StateError),
}

impl Record {
    /// The record of generated puzzle `g` with id `record_id`.
    pub fn new(record_id: u64, g: &GeneratedPuzzle, run: &RunInfo) -> Self {
        let n_tubes = g.state.n_tubes();
        Self {
            record_id,
            generator_id: g.generator_id.to_owned(),
            generator_version: g.generator_version,
            generator_variant: g.generator_variant.clone(),
            layout: run.layout,
            params: g.state.params(),
            gen_config: run.gen_config.clone(),
            seed: g.seed,
            puzzle_code: g.puzzle_code.clone(),
            state: encode_state(&g.state),
            opt_moves: g.opt_moves,
            tier: Tier::of(&run.params, run.layout, g.opt_moves),
            solution: g
                .solution
                .iter()
                .map(|m| u16::try_from(m.action_index(n_tubes)).expect("n_tubes² fits in u16"))
                .collect(),
            canonical_hash: g.canonical_hash,
            attempts: g.attempts,
            metrics: RecordMetrics::from(&g.metrics),
            split: run.split.of(g.canonical_hash),
            created_at: run.created_at,
            tool_version: run.tool_version.clone(),
        }
    }

    /// The stored initial state.
    ///
    /// # Errors
    ///
    /// A wrong cell count, a gap inside a tube, or contents that are not a valid state.
    pub fn decode_state(&self) -> Result<State, StateDecodeError> {
        decode_state(self.params, &self.state)
    }

    /// The stored solution as moves; `None` if an action index is out of range.
    pub fn moves(&self) -> Option<Vec<Move>> {
        let n_tubes = self.params.n_tubes();
        self.solution
            .iter()
            .map(|&a| Move::from_action_index(usize::from(a), n_tubes))
            .collect()
    }
}

/// `n_tubes × capacity` cells, tube by tube, bottom to top, [`EMPTY`] above the fill height.
pub fn encode_state(s: &State) -> Vec<u8> {
    (0..s.n_tubes())
        .flat_map(|i| s.padded_tube(i).iter().copied())
        .collect()
}

/// Inverse of [`encode_state`].
///
/// # Errors
///
/// See [`Record::decode_state`].
pub fn decode_state(params: Params, cells: &[u8]) -> Result<State, StateDecodeError> {
    let cap = usize::from(params.capacity);
    let expected = params.n_tubes() * cap;
    if cells.len() != expected || cap == 0 {
        return Err(StateDecodeError::Length {
            expected,
            got: cells.len(),
        });
    }
    let mut tubes = Vec::with_capacity(params.n_tubes());
    for (i, tube) in cells.chunks(cap).enumerate() {
        let height = tube.iter().position(|&c| c == EMPTY).unwrap_or(cap);
        if tube[height..].iter().any(|&c| c != EMPTY) {
            return Err(StateDecodeError::Floating(i));
        }
        tubes.push(&tube[..height]);
    }
    Ok(State::from_tubes(params, &tubes)?)
}

/// `u64` as a 16-digit lower-case hex string (CLAUDE.md rule 8).
pub mod hex_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's `with` signature
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:016x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        parse(&s).map_err(serde::de::Error::custom)
    }

    /// Parses exactly 16 hex digits.
    ///
    /// # Errors
    ///
    /// Anything else.
    pub fn parse(s: &str) -> Result<u64, String> {
        if s.len() != 16 {
            return Err(format!("expected 16 hex digits, got {s:?}"));
        }
        u64::from_str_radix(s, 16).map_err(|e| format!("invalid hex {s:?}: {e}"))
    }
}

/// Unix nanoseconds as an RFC 3339 string.
mod rfc3339 {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::dataset::time::{format_rfc3339, parse_rfc3339};

    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's `with` signature
    pub fn serialize<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_rfc3339(*v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let s = String::deserialize(d)?;
        parse_rfc3339(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use uniform_water_sort::Uniform;
    use water_sort_core::{GenConfig, Generator, replay};

    pub const P: Params = Params {
        n_colors: 4,
        capacity: 4,
        n_empty: 2,
    };

    pub fn run_info() -> RunInfo {
        RunInfo {
            params: P,
            layout: Layout::Standard,
            gen_config: serde_json::to_string(&GenConfig::default()).unwrap(),
            split: SplitRanges::DEFAULT,
            created_at: 1_791_030_896_000_000_000,
            tool_version: "test".into(),
        }
    }

    /// Record `i` of a small uniform run (seed `i`).
    pub fn sample(i: u64) -> Record {
        let g = Uniform::default()
            .generate(&P, i, &GenConfig::default())
            .unwrap();
        Record::new(i, &g, &run_info())
    }

    #[test]
    fn record_round_trips() {
        let g = Uniform::default()
            .generate(&P, 7, &GenConfig::default())
            .unwrap();
        let r = Record::new(3, &g, &run_info());
        assert_eq!(r.decode_state(), Ok(g.state));
        assert_eq!(r.state.len(), P.n_tubes() * usize::from(P.capacity));
        assert_eq!(r.moves().unwrap(), g.solution);
        assert!(replay(&g.state, &r.moves().unwrap()).unwrap().is_solved());
        assert_eq!(r.tier, Tier::of(&P, Layout::Standard, g.opt_moves));
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(&format!("\"seed\":\"{:016x}\"", 7)));
        assert!(json.contains("\"created_at\":\"2026-10-03T12:34:56.000000000Z\""));
        assert!(json.contains("\"metrics_segments\":"));
        assert_eq!(serde_json::from_str::<Record>(&json).unwrap(), r);
    }

    #[test]
    fn state_decoding_rejects_bad_cells() {
        let r = sample(1);
        let mut short = r.state.clone();
        short.pop();
        assert!(matches!(
            decode_state(P, &short),
            Err(StateDecodeError::Length { .. })
        ));
        let mut floating = r.state.clone();
        // The first empty tube gets a unit above a gap.
        let empty_tube = usize::from(P.n_colors) * usize::from(P.capacity);
        floating[empty_tube + 1] = 0;
        assert_eq!(
            decode_state(P, &floating),
            Err(StateDecodeError::Floating(usize::from(P.n_colors)))
        );
        let mut wrong = r.state;
        wrong[0] = 9;
        assert!(matches!(
            decode_state(P, &wrong),
            Err(StateDecodeError::State(_))
        ));
    }
}
