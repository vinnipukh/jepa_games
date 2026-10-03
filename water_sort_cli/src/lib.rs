//! Library side of `water_sort_cli`: the shared argument types and the dataset pipeline
//! (Phase 4: record schema, JSONL / Parquet I/O, generation, dedup, split, leakage, validation).
//! The binary's subcommands call into it, and its integration tests use it directly.

#![forbid(unsafe_code)]
// An application library: its callers are this package's binary and tests, so `# Errors` and
// `# Panics` sections on every public function would only repeat the code.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod args;
pub mod dataset;
