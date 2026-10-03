//! `generate`: a dataset of `count` puzzles from one generator and configuration.
//!
//! Record `i` uses seed `splitmix64(master_seed ^ i)`, so any record can be regenerated on its
//! own. Indices are generated in parallel (rayon) in chunks of [`CHUNK`]; a window of
//! [`WINDOW_CHUNKS`] chunks is collected in index order and handed through a bounded channel to a
//! single writer thread, which writes while the next window is generated. The written bytes
//! therefore depend only on the records, never on the thread count, and memory stays bounded.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Instant;

use rayon::prelude::*;
use water_sort_core::{GenConfig, GenError, Params, splitmix64};

use super::dedup::{Dedup, DedupCounts, Seen, canonical_encoding, report_markdown};
use super::jsonl::JsonlWriter;
use super::manifest::{
    DedupSummary, Failure, FileEntry, Format, GeneratorInfo, MANIFEST, Manifest, SplitInfo,
    sha256_file,
};
use super::parquet::ParquetWriter;
use super::record::{Record, RunInfo};
use super::split::{Split, SplitRanges};
use super::time::{format_rfc3339, now_unix_nanos};
use crate::args::GenSpec;

/// Dropped duplicates: `dropped_id,kept_id,canonical_hash` per line.
pub const DROPPED_FILE: &str = "duplicates.csv";
/// The duplicate / split / tier report.
pub const REPORT_FILE: &str = "dedup_report.md";

/// Indices per parallel task.
pub const CHUNK: u64 = 256;
/// Chunks generated in parallel before their records go to the writer.
pub const WINDOW_CHUNKS: u64 = 64;
/// Windows that may wait in the channel for the writer.
const CHANNEL_WINDOWS: usize = 2;

/// Seed of record `i` (both generators).
pub const fn record_seed(master_seed: u64, i: u64) -> u64 {
    splitmix64(master_seed ^ i)
}

/// What to generate and where.
#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub spec: GenSpec,
    pub params: Params,
    pub gen_config: GenConfig,
    pub count: u64,
    pub master_seed: u64,
    /// `given` or `fresh`.
    pub master_seed_source: String,
    pub split: SplitRanges,
    pub format: Format,
    /// The `created_at` of every record (Unix nanoseconds, a whole number of microseconds, the
    /// Parquet resolution): one clock reading per run, or pinned by the caller.
    pub created_at: i64,
    pub tool_version: String,
    /// Whether `params` is in the supported range of the layout.
    pub supported: bool,
    /// Worker threads; `None` uses rayon's default (`RAYON_NUM_THREADS` or all cores).
    pub threads: Option<usize>,
    /// Print progress to stderr.
    pub progress: bool,
    /// Indices per parallel task ([`CHUNK`]) and chunks per window ([`WINDOW_CHUNKS`]). They
    /// only trade memory for load balance; the output does not depend on them.
    pub chunk: u64,
    pub window_chunks: u64,
}

/// The outcome of one index.
enum Outcome {
    /// The record and its exact canonical encoding (for dedup).
    Record(Box<Record>, Vec<u8>),
    Failed(Failure),
}

fn generate_chunk(
    opts: &GenerateOptions,
    run: &RunInfo,
    chunk: u64,
) -> Result<Vec<Outcome>, GenError> {
    let start = chunk * opts.chunk;
    let end = (start + opts.chunk).min(opts.count);
    (start..end)
        .map(|i| {
            let seed = record_seed(opts.master_seed, i);
            match opts.spec.generate(&opts.params, seed, &opts.gen_config) {
                Ok(g) => Ok(Outcome::Record(
                    Box::new(Record::new(i, &g, run)),
                    canonical_encoding(&g.state),
                )),
                Err(e @ GenError::TooManyAttempts { .. }) => Ok(Outcome::Failed(Failure {
                    record_id: i,
                    seed,
                    error: e.to_string(),
                })),
                Err(e) => Err(e),
            }
        })
        .collect()
}

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

/// The writer thread's state: files, dedup, counts and the dropped-record log.
struct Writer {
    files: Vec<OutFile>,
    dedup: Dedup,
    dedup_counts: DedupCounts,
    dropped: BufWriter<File>,
    failures: Vec<Failure>,
    generated: u64,
    split_counts: BTreeMap<Split, u64>,
    tiers: BTreeMap<String, u64>,
    split_tiers: BTreeMap<Split, BTreeMap<String, u64>>,
}

fn tier_name(record: &Record) -> String {
    record
        .tier
        .map_or_else(|| "none".to_owned(), |t| t.to_string())
}

impl Writer {
    fn create(dir: &Path, opts: &GenerateOptions) -> io::Result<Self> {
        let name = format!("puzzles.{}", opts.format.extension());
        let files = vec![OutFile {
            sink: Sink::create(&dir.join(&name), opts.format, opts.params)?,
            name,
            split: None,
            records: 0,
        }];
        let mut dropped = BufWriter::new(File::create(dir.join(DROPPED_FILE))?);
        writeln!(dropped, "dropped_id,kept_id,canonical_hash")?;
        Ok(Self {
            files,
            dedup: Dedup::new(),
            dedup_counts: DedupCounts::default(),
            dropped,
            failures: Vec::new(),
            generated: 0,
            split_counts: Split::ALL.iter().map(|&s| (s, 0)).collect(),
            tiers: BTreeMap::new(),
            split_tiers: Split::ALL.iter().map(|&s| (s, BTreeMap::new())).collect(),
        })
    }

    fn accept(&mut self, outcome: Outcome) -> io::Result<()> {
        self.generated += 1;
        let record = match outcome {
            Outcome::Record(r, canonical) => {
                let seen = self.dedup.check(r.canonical_hash, r.record_id, &canonical);
                self.dedup_counts.add(seen);
                if let Seen::Duplicate { kept_id } = seen {
                    writeln!(
                        self.dropped,
                        "{},{kept_id},{:016x}",
                        r.record_id, r.canonical_hash
                    )?;
                    return Ok(());
                }
                *r
            }
            Outcome::Failed(f) => {
                self.failures.push(f);
                return Ok(());
            }
        };
        let tier = tier_name(&record);
        *self.split_counts.entry(record.split).or_default() += 1;
        *self
            .split_tiers
            .entry(record.split)
            .or_default()
            .entry(tier.clone())
            .or_default() += 1;
        *self.tiers.entry(tier).or_default() += 1;
        let file = &mut self.files[0];
        file.records += 1;
        file.sink.write(record)
    }

    fn run(mut self, windows: &Receiver<Vec<Vec<Outcome>>>) -> io::Result<Self> {
        for window in windows {
            for outcome in window.into_iter().flatten() {
                self.accept(outcome)?;
            }
        }
        Ok(self)
    }
}

/// Generates every index on `pool` and feeds the outcomes to `writer` in index order.
fn produce(
    opts: &GenerateOptions,
    run: &RunInfo,
    pool: &rayon::ThreadPool,
    writer: Writer,
) -> io::Result<Writer> {
    let clock = Instant::now();
    let n_chunks = opts.count.div_ceil(opts.chunk.max(1));
    let window_chunks = opts.window_chunks.max(1);
    let (tx, rx) = sync_channel::<Vec<Vec<Outcome>>>(CHANNEL_WINDOWS);
    let (writer, generated) = std::thread::scope(|scope| {
        let handle = scope.spawn(move || writer.run(&rx));
        let mut result = Ok(());
        let mut last_report = Instant::now();
        for window_start in (0..n_chunks).step_by(usize::try_from(window_chunks).unwrap_or(1)) {
            let window_end = (window_start + window_chunks).min(n_chunks);
            let window: Result<Vec<Vec<Outcome>>, GenError> = pool.install(|| {
                (window_start..window_end)
                    .into_par_iter()
                    .map(|c| generate_chunk(opts, run, c))
                    .collect()
            });
            match window {
                Ok(w) => {
                    if tx.send(w).is_err() {
                        break; // The writer failed; its error is returned below.
                    }
                }
                Err(e) => {
                    result = Err(io::Error::other(e));
                    break;
                }
            }
            if opts.progress && last_report.elapsed().as_secs() >= 10 {
                last_report = Instant::now();
                let done = (window_end * opts.chunk).min(opts.count);
                eprintln!(
                    "generated {done} / {} ({:.0} s)",
                    opts.count,
                    clock.elapsed().as_secs_f64()
                );
            }
        }
        drop(tx);
        let writer = handle.join().expect("writer thread panicked");
        (writer, result)
    });
    generated?;
    writer
}

/// Generates the dataset into `dir` (created if missing) and writes its manifest.
///
/// # Errors
///
/// I/O errors, and generator errors other than `TooManyAttempts` (invalid params, a strategy that
/// does not support the layout), which would fail for every index. `TooManyAttempts` is recorded
/// in the manifest's `failures` and the index is skipped.
pub fn generate(dir: &Path, opts: &GenerateOptions) -> io::Result<Manifest> {
    let started_at = now_unix_nanos();
    let clock = Instant::now();
    std::fs::create_dir_all(dir)?;
    let mut pool = rayon::ThreadPoolBuilder::new();
    if let Some(n) = opts.threads {
        pool = pool.num_threads(n);
    }
    let pool = pool.build().map_err(io::Error::other)?;
    let threads = pool.current_num_threads();
    let gen_config_json = serde_json::to_string(&opts.gen_config).map_err(io::Error::other)?;
    let run = RunInfo {
        params: opts.params,
        layout: opts.spec.layout(),
        gen_config: gen_config_json.clone(),
        split: opts.split,
        created_at: opts.created_at,
        tool_version: opts.tool_version.clone(),
    };
    let writer = produce(opts, &run, &pool, Writer::create(dir, opts)?)?;
    writer
        .dropped
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    let mut files = Vec::new();
    for f in writer.files {
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
    let records = files.iter().map(|f| f.records).sum();
    let finished_at = now_unix_nanos();
    let manifest = Manifest {
        format_version: super::manifest::FORMAT_VERSION,
        generator: GeneratorInfo {
            id: opts.spec.id().to_owned(),
            version: opts.spec.version(),
            variant: opts.spec.variant(),
            layout: opts.spec.layout(),
            spec: opts.spec.spec_arg(),
        },
        params: opts.params,
        gen_config: opts.gen_config,
        gen_config_json,
        supported: opts.supported,
        master_seed: opts.master_seed,
        master_seed_source: opts.master_seed_source.clone(),
        seed_rule: "seed_i = splitmix64(master_seed ^ i)".into(),
        count: opts.count,
        records,
        failures: writer.failures,
        dedup: DedupSummary {
            duplicates: writer.dedup_counts.duplicates,
            duplicate_rate: writer.dedup_counts.rate(),
            hash_collisions: writer.dedup_counts.hash_collisions,
            dropped_file: DROPPED_FILE.into(),
            saturation: writer.dedup_counts.curve(),
        },
        split: SplitInfo {
            rule: "canonical_hash % 100".into(),
            ranges: opts.split,
            buckets: Split::ALL
                .iter()
                .map(|&s| (s, opts.split.range(s)))
                .collect(),
            counts: writer.split_counts,
        },
        tiers: writer.tiers,
        split_tiers: writer.split_tiers,
        format: opts.format,
        files,
        tool_version: opts.tool_version.clone(),
        created_at: format_rfc3339(opts.created_at),
        started_at: format_rfc3339(started_at),
        finished_at: format_rfc3339(finished_at),
        wall_time_secs: clock.elapsed().as_secs_f64(),
        threads,
        exclusion: None,
    };
    debug_assert_eq!(writer.generated, opts.count);
    manifest.write(dir)?;
    std::fs::write(dir.join(REPORT_FILE), report_markdown(&manifest))?;
    Ok(manifest)
}

/// Whether `dir` already holds a dataset.
pub fn has_dataset(dir: &Path) -> bool {
    dir.join(MANIFEST).exists()
}

/// Paths of the record files of a dataset, from its manifest.
pub fn record_files(dir: &Path, manifest: &Manifest) -> Vec<PathBuf> {
    manifest.files.iter().map(|f| dir.join(&f.path)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dataset::read_records;
    use uniform_water_sort::Uniform;
    use water_sort_core::Layout;

    pub fn options(count: u64, format: Format, threads: usize) -> GenerateOptions {
        GenerateOptions {
            spec: GenSpec::Uniform(Uniform::new(Layout::Standard)),
            params: Params {
                n_colors: 4,
                capacity: 4,
                n_empty: 2,
            },
            gen_config: GenConfig::default(),
            count,
            master_seed: 0x1234_5678_9abc_def0,
            master_seed_source: "given".into(),
            split: SplitRanges::DEFAULT,
            format,
            created_at: 1_791_030_896_000_000_000,
            tool_version: "test".into(),
            supported: true,
            threads: Some(threads),
            progress: false,
            chunk: CHUNK,
            window_chunks: WINDOW_CHUNKS,
        }
    }

    pub fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ws_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn output_is_independent_of_thread_count() {
        // Several windows of several chunks, so the ordered hand-off is exercised; the chunking
        // must not matter either.
        let count = 230;
        for format in [Format::Parquet, Format::Jsonl] {
            let mut digests = Vec::new();
            for (threads, chunk, window_chunks) in
                [(1, 7, 3), (3, 16, 4), (4, CHUNK, WINDOW_CHUNKS)]
            {
                let dir = temp_dir(&format!("gen_{}_{threads}", format.extension()));
                let opts = GenerateOptions {
                    chunk,
                    window_chunks,
                    ..options(count, format, threads)
                };
                let m = generate(&dir, &opts).unwrap();
                assert_eq!(m.count, count);
                assert_eq!(m.threads, threads);
                digests.push(m.files.clone());
                let records = read_records(&dir, &m).unwrap();
                assert_eq!(records.len() as u64, m.records);
                assert!(records.windows(2).all(|w| w[0].record_id < w[1].record_id));
                let r = &records[5];
                assert_eq!(r.seed, record_seed(0x1234_5678_9abc_def0, r.record_id));
                std::fs::remove_dir_all(&dir).unwrap();
            }
            assert!(digests.windows(2).all(|w| w[0] == w[1]), "{format:?}");
        }
    }

    #[test]
    fn duplicates_are_dropped_keeping_the_lowest_id() {
        // 2 colors / capacity 3 / 1 empty has only a handful of distinct puzzles.
        let mut opts = options(200, Format::Jsonl, 2);
        opts.params = Params {
            n_colors: 2,
            capacity: 3,
            n_empty: 1,
        };
        let dir = temp_dir("gen_dups");
        let m = generate(&dir, &opts).unwrap();
        let records = read_records(&dir, &m).unwrap();
        assert_eq!(m.records + m.dedup.duplicates, 200);
        assert!(m.dedup.duplicates > 150, "{:?}", m.dedup);
        let mut hashes: Vec<u64> = records.iter().map(|r| r.canonical_hash).collect();
        hashes.sort_unstable();
        hashes.dedup();
        assert_eq!(hashes.len(), records.len());
        let dropped = std::fs::read_to_string(dir.join(DROPPED_FILE)).unwrap();
        assert_eq!(dropped.lines().count() as u64, m.dedup.duplicates + 1);
        for line in dropped.lines().skip(1) {
            let mut f = line.split(',');
            let dropped: u64 = f.next().unwrap().parse().unwrap();
            let kept: u64 = f.next().unwrap().parse().unwrap();
            assert!(kept < dropped);
            assert!(records.iter().any(|r| r.record_id == kept));
        }
        assert_eq!(
            m.dedup.saturation.first(),
            Some(&(10, m.dedup.saturation[0].1))
        );
        assert_eq!(m.dedup.saturation.last().unwrap().0, 200);
        let report = std::fs::read_to_string(dir.join(REPORT_FILE)).unwrap();
        assert!(report.contains("| 200 | 0 |"), "{report}");
        let scan = crate::dataset::dedup::scan_stored(&dir, &m).unwrap();
        assert_eq!((scan.counts.seen, scan.counts.duplicates), (m.records, 0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fatal_generator_errors_abort() {
        use turan_water_sort::{Turan, TuranStrategy};
        let mut opts = options(10, Format::Jsonl, 1);
        // Pour walk does not support the standard layout (D16).
        opts.spec = GenSpec::Turan(Turan::new(
            TuranStrategy::DEFAULT_POUR_WALK,
            Layout::Standard,
        ));
        let dir = temp_dir("gen_fatal");
        assert!(generate(&dir, &opts).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        // `TooManyAttempts` is recorded, not fatal.
        let mut opts = options(3, Format::Jsonl, 1);
        opts.gen_config.min_opt = 1000;
        opts.gen_config.max_attempts = 2;
        let m = generate(&dir, &opts).unwrap();
        assert_eq!((m.records, m.failures.len()), (0, 3));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
