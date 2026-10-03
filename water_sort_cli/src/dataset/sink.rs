//! Writing the record files of a dataset directory: one file, or one per split, with per-split
//! and per-tier counts for the manifest.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

use water_sort_core::Params;

use super::jsonl::JsonlWriter;
use super::manifest::{FileEntry, Format, sha256_file};
use super::parquet::ParquetWriter;
use super::record::Record;
use super::split::Split;

/// An open record file.
enum Sink {
    Parquet(Box<ParquetWriter<BufWriter<File>>>),
    Jsonl(JsonlWriter<BufWriter<File>>),
}

impl Sink {
    fn create(path: &Path, format: Format, params: Params) -> io::Result<Self> {
        let out = BufWriter::new(File::create(path)?);
        Ok(match format {
            Format::Parquet => Self::Parquet(Box::new(ParquetWriter::new(out, params)?)),
            Format::Jsonl => Self::Jsonl(JsonlWriter::new(out)),
        })
    }

    fn write(&mut self, record: Record) -> io::Result<()> {
        match self {
            Self::Parquet(w) => w.write(record),
            Self::Jsonl(w) => w.write(&record),
        }
    }

    fn finish(self) -> io::Result<()> {
        let out = match self {
            Self::Parquet(w) => (*w).finish()?,
            Self::Jsonl(w) => w.finish()?,
        };
        out.into_inner().map_err(io::IntoInnerError::into_error)?;
        Ok(())
    }
}

/// One output file and its record count.
struct OutFile {
    name: String,
    split: Option<Split>,
    sink: Sink,
    records: u64,
}

/// The record files of a dataset being written.
pub struct DatasetSink {
    files: Vec<OutFile>,
    counts: Counts,
}

/// Records per split and tier (`none` when a record has no tier).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub split_counts: BTreeMap<Split, u64>,
    pub tiers: BTreeMap<String, u64>,
    pub split_tiers: BTreeMap<Split, BTreeMap<String, u64>>,
}

impl Counts {
    pub fn new() -> Self {
        Self {
            split_counts: Split::ALL.iter().map(|&s| (s, 0)).collect(),
            tiers: BTreeMap::new(),
            split_tiers: Split::ALL.iter().map(|&s| (s, BTreeMap::new())).collect(),
        }
    }

    pub fn add(&mut self, record: &Record) {
        let tier = record
            .tier
            .map_or_else(|| "none".to_owned(), |t| t.to_string());
        *self.split_counts.entry(record.split).or_default() += 1;
        *self
            .split_tiers
            .entry(record.split)
            .or_default()
            .entry(tier.clone())
            .or_default() += 1;
        *self.tiers.entry(tier).or_default() += 1;
    }
}

/// What [`DatasetSink::finish`] wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub files: Vec<FileEntry>,
    pub split_counts: BTreeMap<Split, u64>,
    pub tiers: BTreeMap<String, u64>,
    pub split_tiers: BTreeMap<Split, BTreeMap<String, u64>>,
}

impl Written {
    pub fn records(&self) -> u64 {
        self.files.iter().map(|f| f.records).sum()
    }
}

impl DatasetSink {
    /// Creates `puzzles.<ext>`, or `train.<ext>`, `val.<ext>` and `test.<ext>` with
    /// `split_files`, in `dir`.
    pub fn create(
        dir: &Path,
        format: Format,
        params: Params,
        split_files: bool,
    ) -> io::Result<Self> {
        let ext = format.extension();
        let names: Vec<(String, Option<Split>)> = if split_files {
            Split::ALL
                .iter()
                .map(|&s| (format!("{s}.{ext}"), Some(s)))
                .collect()
        } else {
            vec![(format!("puzzles.{ext}"), None)]
        };
        let files = names
            .into_iter()
            .map(|(name, split)| {
                Ok(OutFile {
                    sink: Sink::create(&dir.join(&name), format, params)?,
                    name,
                    split,
                    records: 0,
                })
            })
            .collect::<io::Result<_>>()?;
        Ok(Self {
            files,
            counts: Counts::new(),
        })
    }

    /// Appends a record to the file of its split.
    pub fn write(&mut self, record: Record) -> io::Result<()> {
        self.counts.add(&record);
        let file = self
            .files
            .iter_mut()
            .find(|f| f.split.is_none_or(|s| s == record.split))
            .expect("a file for every split");
        file.records += 1;
        file.sink.write(record)
    }

    /// Closes every file and hashes it.
    pub fn finish(self, dir: &Path) -> io::Result<Written> {
        let mut files = Vec::new();
        for f in self.files {
            f.sink.finish()?;
            let (sha256, bytes) = sha256_file(&dir.join(&f.name))?;
            files.push(FileEntry {
                path: f.name,
                split: f.split,
                records: f.records,
                bytes,
                sha256,
            });
        }
        Ok(Written {
            files,
            split_counts: self.counts.split_counts,
            tiers: self.counts.tiers,
            split_tiers: self.counts.split_tiers,
        })
    }
}
