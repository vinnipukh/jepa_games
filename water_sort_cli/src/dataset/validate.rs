//! `validate <dir>`: re-checks a dataset against its manifest and the game rules in core.
//!
//! Every record: its fields match the manifest, the seed follows the seed rule, the stored state
//! is valid and equals the decoded puzzle code, the canonical hash is recomputed, the solution
//! replays legally to solved in exactly `opt_moves` moves within the `GenConfig` band, and the
//! tier and split follow their rules. Across records: ids increase within a file and stay below
//! `count`, no canonical form occurs twice, and the counts and file hashes match the manifest.
//! A deterministic sample (default 1 %) is regenerated from its seed and must equal the stored
//! record field for field (this re-runs the solver, hence the sampling).

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use water_sort_core::{GenConfig, Tier, canonical_hash, puzzle_code, replay, splitmix64};

use super::dedup::canonical_encoding;
use super::generate::record_seed;
use super::manifest::{Manifest, sha256_file};
use super::record::{Record, RunInfo};
use super::sink::Counts;
use super::split::Split;
use super::time::parse_rfc3339;
use super::visit_records;
use crate::args::GenSpec;

/// Salt of the regeneration sample.
const SAMPLE_SALT: u64 = 0x7661_6c69_6461_7465; // "validate"

/// Whether record `record_id` is in the regeneration sample at `rate` (0..=1).
pub fn sampled(record_id: u64, rate: f64) -> bool {
    #[allow(clippy::cast_precision_loss)] // 53 bits are exact
    let u = (splitmix64(record_id ^ SAMPLE_SALT) >> 11) as f64 / (1u64 << 53) as f64;
    u < rate
}

/// Errors listed in full; the rest are only counted.
const MAX_LISTED: usize = 50;

/// The outcome of a validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidateReport {
    pub records: u64,
    pub regenerated: u64,
    pub error_count: u64,
    /// The first [`MAX_LISTED`] errors.
    pub errors: Vec<String>,
}

impl ValidateReport {
    pub const fn ok(&self) -> bool {
        self.error_count == 0
    }

    fn error(&mut self, e: String) {
        self.error_count += 1;
        if self.errors.len() < MAX_LISTED {
            self.errors.push(e);
        }
    }

    pub fn summary(&self) -> String {
        let mut s = format!(
            "{} records checked, {} regenerated from their seed: {}\n",
            self.records,
            self.regenerated,
            if self.ok() {
                "OK".to_owned()
            } else {
                format!("{} errors", self.error_count)
            }
        );
        for e in &self.errors {
            let _ = writeln!(s, "  {e}");
        }
        if self.error_count > self.errors.len() as u64 {
            let _ = writeln!(
                s,
                "  ... and {} more",
                self.error_count - self.errors.len() as u64
            );
        }
        s
    }
}

/// Shared, read-only context of the per-record checks.
struct Context {
    manifest: Manifest,
    spec: GenSpec,
    gen_config: GenConfig,
    created_at: i64,
    regen_rate: f64,
}

/// The per-record checks. Returns the errors and whether the record was regenerated.
fn check_record(ctx: &Context, file_split: Option<Split>, r: &Record) -> (Vec<String>, bool) {
    let m = &ctx.manifest;
    let mut errors = Vec::new();
    let mut fail = |what: String| errors.push(format!("record {}: {what}", r.record_id));
    let g = &m.generator;
    if (
        r.generator_id.as_str(),
        r.generator_version,
        r.generator_variant.as_str(),
        r.layout,
    ) != (g.id.as_str(), g.version, g.variant.as_str(), g.layout)
    {
        fail(format!(
            "generator {} v{} {} {} differs from the manifest",
            r.generator_id, r.generator_version, r.generator_variant, r.layout
        ));
    }
    if r.params != m.params {
        fail(format!("params {:?} differ from the manifest", r.params));
    }
    if r.gen_config != m.gen_config_json {
        fail("gen_config differs from the manifest".into());
    }
    if r.created_at != ctx.created_at {
        fail("created_at differs from the manifest".into());
    }
    if r.seed != record_seed(m.master_seed, r.record_id) {
        fail(format!(
            "seed {:016x} does not follow the seed rule",
            r.seed
        ));
    }
    let state = match r.decode_state() {
        Ok(s) => s,
        Err(e) => {
            fail(format!("invalid state: {e}"));
            return (errors, false);
        }
    };
    match puzzle_code::decode(&r.puzzle_code) {
        Ok(s) if s == state => {}
        Ok(_) => fail("puzzle code decodes to a different state".into()),
        Err(e) => fail(format!("invalid puzzle code: {e}")),
    }
    if canonical_hash(&state) != r.canonical_hash {
        fail("canonical hash does not match the state".into());
    }
    match r.moves() {
        None => fail("solution has an out-of-range action".into()),
        Some(moves) => {
            if moves.len() != r.opt_moves as usize {
                fail(format!(
                    "solution has {} moves, opt_moves is {}",
                    moves.len(),
                    r.opt_moves
                ));
            }
            match replay(&state, &moves) {
                Some(end) if end.is_solved() => {}
                Some(_) => fail("solution does not solve the puzzle".into()),
                None => fail("solution contains an illegal move".into()),
            }
        }
    }
    let cfg = &ctx.gen_config;
    if r.opt_moves < cfg.min_opt || cfg.max_opt.is_some_and(|x| r.opt_moves > x) {
        fail(format!(
            "opt_moves {} is outside the configured band",
            r.opt_moves
        ));
    }
    if r.tier != Tier::of(&r.params, r.layout, r.opt_moves) {
        fail(format!("tier {:?} does not match opt_moves", r.tier));
    }
    if r.split != m.split.ranges.of(r.canonical_hash) {
        fail(format!(
            "split {} does not match the canonical hash",
            r.split
        ));
    }
    if file_split.is_some_and(|s| s != r.split) {
        fail(format!("{} record in a {file_split:?} file", r.split));
    }
    if r.metrics.random_rollouts != cfg.metrics.random_rollouts {
        fail("metrics_random_rollouts differs from gen_config".into());
    }
    let regen = sampled(r.record_id, ctx.regen_rate);
    if regen && let Some(e) = regenerate(ctx, r) {
        fail(e);
    }
    (errors, regen)
}

/// Regenerates `r` from its seed; `Some(error)` if the result differs.
fn regenerate(ctx: &Context, r: &Record) -> Option<String> {
    let m = &ctx.manifest;
    match ctx.spec.generate(&m.params, r.seed, &ctx.gen_config) {
        Ok(g) => {
            let run = RunInfo {
                params: m.params,
                layout: m.generator.layout,
                gen_config: m.gen_config_json.clone(),
                split: m.split.ranges,
                created_at: r.created_at,
                tool_version: r.tool_version.clone(),
            };
            (Record::new(r.record_id, &g, &run) != *r)
                .then(|| "regenerating from the seed gives a different record".into())
        }
        Err(e) => Some(format!("regenerating from the seed fails: {e}")),
    }
}

/// Manifest-level checks: the generator still matches this build, the config JSON, the file
/// hashes. Returns the generator spec if it parses.
fn check_manifest(dir: &Path, manifest: &Manifest, report: &mut ValidateReport) -> Option<GenSpec> {
    let spec: GenSpec = match manifest.generator.spec.parse() {
        Ok(s) => s,
        Err(e) => {
            report.error(format!("manifest generator spec: {e}"));
            return None;
        }
    };
    let g = &manifest.generator;
    if (spec.id(), spec.version(), spec.variant(), spec.layout())
        != (g.id.as_str(), g.version, g.variant.clone(), g.layout)
    {
        report.error(format!(
            "this build's generator {} v{} {} differs from the manifest's {} v{} {}; \
             records cannot be regenerated",
            spec.id(),
            spec.version(),
            spec.variant(),
            g.id,
            g.version,
            g.variant
        ));
    }
    match serde_json::from_str::<GenConfig>(&manifest.gen_config_json) {
        Ok(c) if c == manifest.gen_config => {}
        _ => report.error("manifest gen_config_json does not match gen_config".into()),
    }
    for f in &manifest.files {
        match sha256_file(&dir.join(&f.path)) {
            Ok((sha, bytes)) if (sha.as_str(), bytes) == (f.sha256.as_str(), f.bytes) => {}
            Ok(_) => report.error(format!(
                "{}: sha256 or size differs from the manifest",
                f.path
            )),
            Err(e) => report.error(format!("{}: {e}", f.path)),
        }
    }
    Some(spec)
}

/// Validates the dataset in `dir`, regenerating a `regen_rate` fraction of the records.
///
/// # Errors
///
/// Only when the manifest or a record file cannot be read at all; everything else is reported
/// in the [`ValidateReport`].
pub fn validate(dir: &Path, regen_rate: f64) -> io::Result<ValidateReport> {
    let manifest = Manifest::read(dir)?;
    let mut report = ValidateReport::default();
    let Some(spec) = check_manifest(dir, &manifest, &mut report) else {
        return Ok(report);
    };
    let created_at = parse_rfc3339(&manifest.created_at).unwrap_or_else(|e| {
        report.error(format!("manifest created_at: {e}"));
        0
    });
    let ctx = Context {
        gen_config: manifest.gen_config,
        manifest: manifest.clone(),
        spec,
        created_at,
        regen_rate,
    };
    let failed: rustc_hash::FxHashSet<u64> =
        manifest.failures.iter().map(|f| f.record_id).collect();
    let mut hashes: FxHashMap<u64, (u64, Vec<u8>)> = FxHashMap::default();
    let mut counts = Counts::new();
    let mut file_records: FxHashMap<String, u64> = FxHashMap::default();
    let mut last_id: FxHashMap<String, u64> = FxHashMap::default();
    visit_records(dir, &manifest, |entry, batch| {
        let checked: Vec<(Vec<String>, bool, Option<Vec<u8>>)> = batch
            .par_iter()
            .map(|r| {
                let (errors, regen) = check_record(&ctx, entry.split, r);
                let canonical = r.decode_state().ok().map(|s| canonical_encoding(&s));
                (errors, regen, canonical)
            })
            .collect();
        for (r, (errors, regen, canonical)) in batch.iter().zip(checked) {
            report.records += 1;
            report.regenerated += u64::from(regen);
            for e in errors {
                report.error(e);
            }
            *file_records.entry(entry.path.clone()).or_default() += 1;
            if let Some(&prev) = last_id.get(&entry.path)
                && r.record_id <= prev
            {
                report.error(format!(
                    "record {}: ids do not increase in {} (after {prev})",
                    r.record_id, entry.path
                ));
            }
            last_id.insert(entry.path.clone(), r.record_id);
            if r.record_id >= manifest.count || failed.contains(&r.record_id) {
                report.error(format!(
                    "record {}: not a generated index of this run",
                    r.record_id
                ));
            }
            if let Some(canonical) = canonical {
                if let Some((first, enc)) = hashes.get(&r.canonical_hash) {
                    if *enc == canonical {
                        report.error(format!(
                            "record {}: duplicate of record {first}",
                            r.record_id
                        ));
                    }
                } else {
                    hashes.insert(r.canonical_hash, (r.record_id, canonical));
                }
            }
            counts.add(r);
        }
        Ok(())
    })?;
    if report.records != manifest.records {
        report.error(format!(
            "{} records stored, the manifest lists {}",
            report.records, manifest.records
        ));
    }
    for f in &manifest.files {
        let n = file_records.get(&f.path).copied().unwrap_or_default();
        if n != f.records {
            report.error(format!(
                "{}: {n} records, the manifest lists {}",
                f.path, f.records
            ));
        }
    }
    if counts.split_counts != manifest.split.counts
        || counts.tiers != manifest.tiers
        || counts.split_tiers != manifest.split_tiers
    {
        report.error("split or tier counts differ from the manifest".into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::generate::generate;
    use crate::dataset::generate::tests::{options, temp_dir};
    use crate::dataset::manifest::Format;

    #[test]
    fn sample_rate() {
        let n = (0..100_000).filter(|&i| sampled(i, 0.01)).count();
        assert!((800..1200).contains(&n), "{n}");
        assert!((0..1000).all(|i| sampled(i, 1.0)));
        assert!((0..1000).all(|i| !sampled(i, 0.0)));
    }

    #[test]
    fn accepts_a_generated_dataset_and_catches_tampering() {
        for format in [Format::Parquet, Format::Jsonl] {
            let dir = temp_dir(&format!("validate_{}", format.extension()));
            let opts = crate::dataset::generate::GenerateOptions {
                split_files: true,
                ..options(60, format, 2)
            };
            let m = generate(&dir, &opts).unwrap();
            let report = validate(&dir, 1.0).unwrap();
            assert!(report.ok(), "{}", report.summary());
            assert_eq!((report.records, report.regenerated), (m.records, m.records));

            // A changed record is caught, both by its checks and by the file hash.
            let mut records = crate::dataset::read_records(&dir, &m).unwrap();
            let victim = records
                .iter_mut()
                .find(|r| r.split == Split::Train)
                .unwrap();
            victim.opt_moves += 1;
            let mut sink =
                crate::dataset::sink::DatasetSink::create(&dir, format, m.params, true).unwrap();
            for r in records {
                sink.write(r).unwrap();
            }
            sink.finish(&dir).unwrap();
            let report = validate(&dir, 0.0).unwrap();
            let text = report.summary();
            assert!(!report.ok());
            assert!(text.contains("sha256"), "{text}");
            assert!(text.contains("solution has"), "{text}");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
