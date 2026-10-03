//! Puzzle datasets (Phase 4): the record schema, JSONL and Parquet storage, generation with
//! ordered parallel writing, dedup, split, leakage check and validation.

pub mod jsonl;
pub mod record;
pub mod split;
pub mod time;

pub use record::{Record, RecordMetrics, RunInfo};
pub use split::{Split, SplitRanges};
