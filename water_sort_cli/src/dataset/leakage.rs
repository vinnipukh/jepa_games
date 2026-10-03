//! `leakage`: puzzles shared between two datasets (e.g. uniform's test split and Turan's train
//! split), by canonical form, and optionally a copy of one dataset without them.
//!
//! Within one dataset the split rule makes the overlap between splits zero by construction.
//! Across generators the same puzzle can occur in both, and must be removed from the training
//! side for a clean cross-evaluation.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use rustc_hash::FxHashMap;

use super::dedup::{canonical_encoding, report_markdown};
use super::generate::{DROPPED_FILE, REPORT_FILE};
use super::manifest::{Exclusion, MANIFEST, Manifest, sha256_file};
use super::record::Record;
use super::sink::DatasetSink;
use super::split::Split;
use super::visit_records;

/// Which side of a leakage check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Side {
    A,
    B,
}

/// One side: a dataset directory and the split looked at (`None`: all records).
#[derive(Clone, Copy, Debug)]
pub struct Selection<'a> {
    pub dir: &'a Path,
    pub split: Option<Split>,
}

impl Selection<'_> {
    fn label(&self) -> String {
        format!(
            "{} ({})",
            self.dir.display(),
            self.split
                .map_or_else(|| "all splits".to_owned(), |s| s.to_string())
        )
    }

    fn includes(&self, r: &Record) -> bool {
        self.split.is_none_or(|s| s == r.split)
    }
}

/// Overlap between two selections.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leakage {
    pub a_records: u64,
    pub b_records: u64,
    /// Distinct canonical forms present in both.
    pub shared: u64,
    /// Records of A, resp. B, whose canonical form is in the other selection.
    pub a_shared_records: u64,
    pub b_shared_records: u64,
    /// Equal 64-bit hashes with different canonical forms (not counted as shared).
    pub hash_collisions: u64,
}

fn canonical_of(r: &Record) -> io::Result<Vec<u8>> {
    r.decode_state()
        .map(|s| canonical_encoding(&s))
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("record {}: {e}", r.record_id),
            )
        })
}

/// A's canonical forms: hash → (encoding, records of A with it, seen in B).
type Index = FxHashMap<u64, (Vec<u8>, u64, bool)>;

/// Computes the overlap. Returns the counts and the set of shared hashes (confirmed by encoding).
pub fn leakage(a: Selection, b: Selection) -> io::Result<(Leakage, Vec<u64>)> {
    let ma = Manifest::read(a.dir)?;
    let mb = Manifest::read(b.dir)?;
    let mut out = Leakage::default();
    let mut index: Index = FxHashMap::default();
    visit_records(a.dir, &ma, |_, batch| {
        for r in batch.iter().filter(|r| a.includes(r)) {
            out.a_records += 1;
            let enc = canonical_of(r)?;
            let entry = index.entry(r.canonical_hash).or_insert((enc, 0, false));
            entry.1 += 1;
        }
        Ok(())
    })?;
    visit_records(b.dir, &mb, |_, batch| {
        for r in batch.iter().filter(|r| b.includes(r)) {
            out.b_records += 1;
            if let Some(entry) = index.get_mut(&r.canonical_hash) {
                if entry.0 == canonical_of(r)? {
                    out.b_shared_records += 1;
                    entry.2 = true;
                } else {
                    out.hash_collisions += 1;
                }
            }
        }
        Ok(())
    })?;
    let mut shared: Vec<u64> = index
        .iter()
        .filter(|(_, e)| e.2)
        .map(|(&h, e)| {
            out.a_shared_records += e.1;
            h
        })
        .collect();
    shared.sort_unstable();
    out.shared = shared.len() as u64;
    Ok((out, shared))
}

#[allow(clippy::cast_precision_loss)]
fn pct(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        100.0 * num as f64 / den as f64
    }
}

/// The Markdown report of a leakage check.
pub fn report_markdown_leakage(a: Selection, b: Selection, l: &Leakage) -> String {
    let mut md = String::from("# Leakage check\n\n");
    let _ = writeln!(md, "- A: {}", a.label());
    let _ = writeln!(md, "- B: {}\n", b.label());
    md.push_str("| | records | records shared | share |\n|---|---:|---:|---:|\n");
    let _ = writeln!(
        md,
        "| A | {} | {} | {:.4} % |",
        l.a_records,
        l.a_shared_records,
        pct(l.a_shared_records, l.a_records)
    );
    let _ = writeln!(
        md,
        "| B | {} | {} | {:.4} % |\n",
        l.b_records,
        l.b_shared_records,
        pct(l.b_shared_records, l.b_records)
    );
    let _ = writeln!(
        md,
        "{} distinct puzzles (canonical forms, D8) occur in both; {} hash collisions \
         (equal 64-bit hash, different puzzle) were not counted.",
        l.shared, l.hash_collisions
    );
    md
}

/// Writes a copy of the dataset in `source` to `out` without the records of `split` (all
/// splits when `None`) whose canonical hash is in `shared` (sorted), with an updated manifest
/// recording the exclusion. Returns the number of records removed.
pub fn write_excluded(
    source: Selection,
    other: Selection,
    shared: &[u64],
    out: &Path,
) -> io::Result<u64> {
    let m = Manifest::read(source.dir)?;
    std::fs::create_dir_all(out)?;
    let split_files = m.files.iter().any(|f| f.split.is_some());
    let mut sink = DatasetSink::create(out, m.format, m.params, split_files)?;
    let mut removed = 0;
    visit_records(source.dir, &m, |_, batch| {
        for r in batch {
            if source.includes(&r) && shared.binary_search(&r.canonical_hash).is_ok() {
                removed += 1;
            } else {
                sink.write(r)?;
            }
        }
        Ok(())
    })?;
    let written = sink.finish(out)?;
    let dropped = source.dir.join(DROPPED_FILE);
    if dropped.exists() {
        std::fs::copy(dropped, out.join(DROPPED_FILE))?;
    }
    let mut new = m.clone();
    new.records = written.records();
    new.files = written.files;
    new.split.counts = written.split_counts;
    new.tiers = written.tiers;
    new.split_tiers = written.split_tiers;
    new.exclusion = Some(Exclusion {
        source: source.dir.display().to_string(),
        source_manifest_sha256: sha256_file(&source.dir.join(MANIFEST))?.0,
        other: other.dir.display().to_string(),
        other_manifest_sha256: sha256_file(&other.dir.join(MANIFEST))?.0,
        other_split: other.split,
        split: source.split,
        removed,
    });
    new.write(out)?;
    std::fs::write(out.join(REPORT_FILE), report_markdown(&new))?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::generate::tests::{options, temp_dir};
    use crate::dataset::generate::{GenerateOptions, generate};
    use crate::dataset::manifest::Format;
    use crate::dataset::validate::validate;
    use water_sort_core::Params;

    #[test]
    fn finds_and_removes_shared_puzzles() {
        // A tiny configuration, so two runs with different seeds share many puzzles.
        let small = |seed, format| GenerateOptions {
            params: Params {
                n_colors: 3,
                capacity: 3,
                n_empty: 2,
            },
            master_seed: seed,
            split_files: true,
            ..options(150, format, 2)
        };
        let (da, db, dx) = (temp_dir("leak_a"), temp_dir("leak_b"), temp_dir("leak_x"));
        generate(&da, &small(1, Format::Parquet)).unwrap();
        generate(&db, &small(2, Format::Jsonl)).unwrap();
        let a = Selection {
            dir: &da,
            split: None,
        };
        let b = Selection {
            dir: &db,
            split: Some(Split::Train),
        };
        let (l, shared) = leakage(a, b).unwrap();
        assert!(l.shared > 0 && l.b_shared_records == l.shared, "{l:?}");
        assert_eq!(l.a_shared_records, l.shared); // both are deduplicated
        // Within one dataset, test and train never share a puzzle.
        let (own, _) = leakage(
            Selection {
                dir: &da,
                split: Some(Split::Test),
            },
            Selection {
                dir: &da,
                split: Some(Split::Train),
            },
        )
        .unwrap();
        assert_eq!(own.shared, 0);

        let removed = write_excluded(b, a, &shared, &dx).unwrap();
        assert_eq!(removed, l.b_shared_records);
        let (after, _) = leakage(
            a,
            Selection {
                dir: &dx,
                split: Some(Split::Train),
            },
        )
        .unwrap();
        assert_eq!(after.shared, 0);
        assert_eq!(after.b_records, l.b_records - removed);
        let report = validate(&dx, 1.0).unwrap();
        assert!(report.ok(), "{}", report.summary());
        let m = Manifest::read(&dx).unwrap();
        assert_eq!(m.exclusion.unwrap().removed, removed);
        assert!(report_markdown_leakage(a, b, &l).contains("distinct puzzles"));
        for d in [da, db, dx] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }
}
