//! Puzzle datasets (Phase 4): the record schema, JSONL and Parquet storage, generation with
//! ordered parallel writing, dedup, split, leakage check and validation.

pub mod commands;
pub mod dedup;
pub mod generate;
pub mod jsonl;
pub mod leakage;
pub mod manifest;
pub mod parquet;
pub mod record;
pub mod sink;
pub mod split;
pub mod time;
pub mod validate;

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use ::parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

pub use manifest::{FileEntry, Format, Manifest};
pub use record::{Record, RecordMetrics, RunInfo};
pub use split::{Split, SplitRanges};

/// Records handed to a visitor at a time.
const VISIT_BATCH: usize = 64 * 1024;

/// Calls `f` with every record file's records, file by file in manifest order, in batches of at
/// most [`VISIT_BATCH`], so a large dataset never has to fit in memory.
pub fn visit_records(
    dir: &Path,
    manifest: &Manifest,
    mut f: impl FnMut(&FileEntry, Vec<Record>) -> io::Result<()>,
) -> io::Result<()> {
    for entry in &manifest.files {
        let path = dir.join(&entry.path);
        let context = |e: &dyn std::fmt::Display| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        };
        match manifest.format {
            Format::Parquet => {
                let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(&path)?)
                    .map(|b| b.with_batch_size(VISIT_BATCH))
                    .and_then(ParquetRecordBatchReaderBuilder::build)
                    .map_err(|e| context(&e))?;
                for batch in reader {
                    let batch = batch.map_err(|e| context(&e))?;
                    f(entry, parquet::from_batch(&batch).map_err(|e| context(&e))?)?;
                }
            }
            Format::Jsonl => {
                let mut batch = Vec::with_capacity(VISIT_BATCH);
                for (i, line) in BufReader::new(File::open(&path)?).lines().enumerate() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let record = serde_json::from_str(&line)
                        .map_err(|e| context(&format!("line {}: {e}", i + 1)))?;
                    batch.push(record);
                    if batch.len() == VISIT_BATCH {
                        f(entry, std::mem::take(&mut batch))?;
                    }
                }
                if !batch.is_empty() {
                    f(entry, batch)?;
                }
            }
        }
    }
    Ok(())
}

/// Every record of a dataset in memory, file by file in manifest order. For small datasets and
/// tests; large ones should use [`visit_records`].
pub fn read_records(dir: &Path, manifest: &Manifest) -> io::Result<Vec<Record>> {
    let mut all = Vec::new();
    visit_records(dir, manifest, |_, batch| {
        all.extend(batch);
        Ok(())
    })?;
    Ok(all)
}
