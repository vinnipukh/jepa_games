//! Dedup on the canonical hash (D8), and the duplicate report.
//!
//! The first record of every canonical form is kept (records arrive in `record_id` order, so it
//! is the lowest id) and later ones are dropped. A hash match is confirmed by comparing the exact
//! canonical encodings, so a 64-bit collision can only keep a record that should have been
//! dropped, never drop a distinct puzzle.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use rustc_hash::FxHashMap;
use water_sort_core::canon::encode;
use water_sort_core::{State, canonical_full};

use super::manifest::Manifest;
use super::record::Record;
use super::split::Split;
use super::visit_records;

/// The exact canonical encoding of a state (what the canonical hash hashes).
pub fn canonical_encoding(state: &State) -> Vec<u8> {
    encode(&canonical_full(state))
}

/// What [`Dedup::check`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    /// First record with this canonical form: keep it.
    New,
    /// Same canonical form as the kept record `kept_id`: drop it.
    Duplicate { kept_id: u64 },
    /// Same 64-bit hash as an earlier record but a different canonical form: keep it.
    HashCollision,
}

/// Canonical forms seen so far. Memory: one hash-map entry plus one encoding per kept record
/// (about 50 bytes at 6 colors / capacity 4 / 2 empty, so about 50 MB for 1M records). Only
/// lookups are made, so the map's iteration order never matters (CLAUDE.md rule 3).
#[derive(Default)]
pub struct Dedup {
    index: FxHashMap<u64, usize>,
    /// Encodings of the kept records, `stride` bytes each.
    arena: Vec<u8>,
    stride: usize,
    kept_ids: Vec<u64>,
}

impl Dedup {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks one record (in `record_id` order) and remembers it if it is new.
    pub fn check(&mut self, hash: u64, record_id: u64, canonical: &[u8]) -> Seen {
        if self.kept_ids.is_empty() {
            self.stride = canonical.len();
        }
        assert_eq!(
            canonical.len(),
            self.stride,
            "all records of a dataset have the same params"
        );
        if let Some(&slot) = self.index.get(&hash) {
            let start = slot * self.stride;
            return if self.arena[start..start + self.stride] == *canonical {
                Seen::Duplicate {
                    kept_id: self.kept_ids[slot],
                }
            } else {
                Seen::HashCollision
            };
        }
        self.index.insert(hash, self.kept_ids.len());
        self.arena.extend_from_slice(canonical);
        self.kept_ids.push(record_id);
        Seen::New
    }
}

/// Running dedup counts, with the saturation curve.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DedupCounts {
    pub seen: u64,
    pub duplicates: u64,
    pub hash_collisions: u64,
    /// `(seen, duplicates)` whenever `seen` reaches a power of ten (from 10).
    pub saturation: Vec<(u64, u64)>,
}

impl DedupCounts {
    pub fn add(&mut self, seen: Seen) {
        self.seen += 1;
        match seen {
            Seen::New => {}
            Seen::Duplicate { .. } => self.duplicates += 1,
            Seen::HashCollision => self.hash_collisions += 1,
        }
        if is_power_of_ten(self.seen) {
            self.saturation.push((self.seen, self.duplicates));
        }
    }

    /// The saturation curve with the final count appended if it is not a power of ten.
    pub fn curve(&self) -> Vec<(u64, u64)> {
        let mut c = self.saturation.clone();
        if self.seen > 0 && c.last().is_none_or(|&(n, _)| n != self.seen) {
            c.push((self.seen, self.duplicates));
        }
        c
    }

    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    pub fn rate(&self) -> f64 {
        if self.seen == 0 {
            0.0
        } else {
            self.duplicates as f64 / self.seen as f64
        }
    }
}

/// 10, 100, 1000, ...
const fn is_power_of_ten(mut n: u64) -> bool {
    if n < 10 {
        return false;
    }
    while n.is_multiple_of(10) {
        n /= 10;
    }
    n == 1
}

#[allow(clippy::cast_precision_loss)]
fn pct(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        100.0 * num as f64 / den as f64
    }
}

/// The duplicate report of a generated dataset, from its manifest.
pub fn report_markdown(manifest: &Manifest) -> String {
    let mut md = String::new();
    duplicates_section(&mut md, manifest);
    split_section(&mut md, manifest);
    tiers_section(&mut md, manifest);
    md.push_str(
        "\n## Files\n\n| file | split | records | bytes | sha256 |\n|---|---|---:|---:|---|\n",
    );
    for f in &manifest.files {
        let split = f.split.map_or_else(|| "all".to_owned(), |s| s.to_string());
        let _ = writeln!(
            md,
            "| `{}` | {split} | {} | {} | `{}` |",
            f.path, f.records, f.bytes, f.sha256
        );
    }
    md
}

fn duplicates_section(md: &mut String, manifest: &Manifest) {
    let (g, params) = (&manifest.generator, manifest.params);
    let _ = writeln!(
        md,
        "# Dataset report: {} / {} colors, capacity {}, {} empty ({})\n",
        g.variant, params.n_colors, params.capacity, params.n_empty, g.layout
    );
    let _ = writeln!(
        md,
        "Generator `{}` v{} (`{}`), master seed `{:016x}` ({}), {}. `{}`. Tool: {}. \
         Wall time {:.1} s on {} threads.\n",
        g.id,
        g.version,
        g.spec,
        manifest.master_seed,
        manifest.master_seed_source,
        manifest.seed_rule,
        manifest.gen_config_json,
        manifest.tool_version,
        manifest.wall_time_secs,
        manifest.threads
    );
    md.push_str("## Duplicates\n\n");
    md.push_str(
        "| generated | failed | duplicates dropped | duplicate rate | hash collisions | records |\n",
    );
    md.push_str("|---:|---:|---:|---:|---:|---:|\n");
    let dedup = &manifest.dedup;
    let _ = writeln!(
        md,
        "| {} | {} | {} | {:.4} % | {} | {} |\n",
        manifest.count,
        manifest.failures.len(),
        dedup.duplicates,
        100.0 * dedup.duplicate_rate,
        dedup.hash_collisions,
        manifest.records
    );
    let _ = writeln!(
        md,
        "Duplicates are records whose canonical form (tube order and color names ignored, D8) \
         equals an earlier record's; the lowest `record_id` is kept. Dropped ids are listed in \
         `{}`. The saturation curve shows how the duplicate count grows with the number of \
         generated puzzles: a rate that rises with `count` means the configuration's puzzle \
         space is being exhausted.\n",
        dedup.dropped_file
    );
    md.push_str("| generated | duplicates | rate |\n|---:|---:|---:|\n");
    for &(n, d) in &dedup.saturation {
        let _ = writeln!(md, "| {n} | {d} | {:.4} % |", pct(d, n));
    }
}

fn split_section(md: &mut String, manifest: &Manifest) {
    let split = &manifest.split;
    md.push_str("\n## Split\n\n");
    let _ = writeln!(
        md,
        "Rule: bucket = `{}`; ranges {}.\n",
        split.rule, split.ranges
    );
    md.push_str("| split | buckets | records | share |\n|---|---|---:|---:|\n");
    for s in Split::ALL {
        let (start, end) = split.buckets.get(&s).copied().unwrap_or_default();
        let n = split.counts.get(&s).copied().unwrap_or_default();
        let _ = writeln!(
            md,
            "| {s} | [{start}, {end}) | {n} | {:.2} % |",
            pct(n, manifest.records)
        );
    }
}

fn tiers_section(md: &mut String, manifest: &Manifest) {
    md.push_str("\n## Tiers (D16)\n\n");
    let mut tiers: Vec<&String> = manifest.tiers.keys().collect();
    tiers.sort_by_key(|t| {
        ["easy", "medium", "hard", "none"]
            .iter()
            .position(|x| x == t)
    });
    md.push_str("| split |");
    for t in &tiers {
        let _ = write!(md, " {t} |");
    }
    md.push_str("\n|---|");
    md.push_str(&"---:|".repeat(tiers.len()));
    md.push('\n');
    let mut rows: Vec<(String, &BTreeMap<String, u64>)> = manifest
        .split_tiers
        .iter()
        .map(|(s, t)| (s.to_string(), t))
        .collect();
    rows.push(("all".into(), &manifest.tiers));
    for (name, counts) in rows {
        let total: u64 = counts.values().sum();
        let _ = write!(md, "| {name} |");
        for t in &tiers {
            let n = counts.get(*t).copied().unwrap_or_default();
            let _ = write!(md, " {n} ({:.1} %) |", pct(n, total));
        }
        md.push('\n');
    }
}

/// Duplicates among the stored records of a dataset (normally none, since `generate` dedups),
/// recomputed from the data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoredScan {
    pub counts: DedupCounts,
    /// `(dropped_id, kept_id)` for every stored duplicate.
    pub duplicates: Vec<(u64, u64)>,
    pub split_counts: BTreeMap<Split, u64>,
}

/// Scans the stored records for duplicates. Records are visited in file order; across split
/// files the first occurrence by file order is the one kept.
pub fn scan_stored(dir: &Path, manifest: &Manifest) -> io::Result<StoredScan> {
    let mut dedup = Dedup::new();
    let mut scan = StoredScan::default();
    visit_records(dir, manifest, |_, batch: Vec<Record>| {
        for r in batch {
            let state = r.decode_state().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("record {}: {e}", r.record_id),
                )
            })?;
            let seen = dedup.check(r.canonical_hash, r.record_id, &canonical_encoding(&state));
            if let Seen::Duplicate { kept_id } = seen {
                scan.duplicates.push((r.record_id, kept_id));
            }
            scan.counts.add(seen);
            *scan.split_counts.entry(r.split).or_default() += 1;
        }
        Ok(())
    })?;
    Ok(scan)
}

/// `dedup-report <dir>`: the generation-time report plus a fresh scan of the stored data.
pub fn standalone_report(dir: &Path) -> io::Result<String> {
    let m = Manifest::read(dir)?;
    let scan = scan_stored(dir, &m)?;
    let mut md = report_markdown(&m);
    md.push_str("\n## Stored data (recomputed)\n\n");
    let _ = writeln!(
        md,
        "{} stored records, {} duplicates among them, {} hash collisions; split counts {}.",
        scan.counts.seen,
        scan.counts.duplicates,
        scan.counts.hash_collisions,
        Split::ALL
            .iter()
            .map(|s| format!(
                "{s} {}",
                scan.split_counts.get(s).copied().unwrap_or_default()
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if scan.counts.seen != m.records {
        let _ = writeln!(
            md,
            "\n**Mismatch:** the manifest lists {} records.",
            m.records
        );
    }
    for (dropped, kept) in scan.duplicates.iter().take(20) {
        let _ = writeln!(md, "- record {dropped} duplicates record {kept}");
    }
    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_confirms_with_the_encoding() {
        let mut d = Dedup::new();
        let mut c = DedupCounts::default();
        let mut add = |d: &mut Dedup, hash, id, enc: &[u8]| {
            let s = d.check(hash, id, enc);
            c.add(s);
            s
        };
        assert_eq!(add(&mut d, 1, 0, &[1, 2]), Seen::New);
        assert_eq!(add(&mut d, 2, 1, &[2, 2]), Seen::New);
        assert_eq!(add(&mut d, 1, 2, &[1, 2]), Seen::Duplicate { kept_id: 0 });
        // Same hash, different canonical form: a collision, kept.
        assert_eq!(add(&mut d, 2, 3, &[3, 3]), Seen::HashCollision);
        for id in 4..12 {
            add(&mut d, 2, id, &[2, 2]);
        }
        assert_eq!((c.seen, c.duplicates, c.hash_collisions), (12, 9, 1));
        assert_eq!(c.saturation, vec![(10, 7)]);
        assert_eq!(c.curve(), vec![(10, 7), (12, 9)]);
        assert!((c.rate() - 0.75).abs() < 1e-12);
    }

    #[test]
    fn saturation_points_are_powers_of_ten() {
        let mut c = DedupCounts::default();
        for _ in 0..1000 {
            c.add(Seen::New);
        }
        assert_eq!(c.saturation, vec![(10, 0), (100, 0), (1000, 0)]);
        assert_eq!(c.curve(), c.saturation);
        assert!(!is_power_of_ten(11) && !is_power_of_ten(1) && !is_power_of_ten(110));
        assert!(is_power_of_ten(1_000_000));
    }
}
