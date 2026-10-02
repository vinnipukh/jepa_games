//! `water_sort_cli stats`: generation statistics per `(n_colors, capacity, n_empty)` cell, written
//! as CSV and Markdown. They fix the supported configuration range (D3).
//!
//! Sample `i` of every cell uses seed `splitmix64(base_seed ^ i)`. Samples run in parallel in
//! fixed-size batches and are collected in index order, so every column except the timing ones
//! is deterministic, independent of the thread count. A cell stops early once its timeout rate
//! clearly exceeds the D3 limit (a deterministic, count-based rule) or its time budget runs out.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Args;
use rayon::prelude::*;
use uniform_water_sort::Uniform;
use water_sort_core::{
    Evaluation, GenConfig, Generator, MetricsConfig, Observer, Params, RejectionCounts,
    SolverLimits, is_symmetric, splitmix64,
};

use crate::args::{GeneratorKind, Range, parse_count};

/// D3 criterion (proposed): solver p99 below this, per attempt.
pub const P99_LIMIT_MS: f64 = 1000.0;
/// D3 criterion (proposed): timeout rate below this, per attempt.
pub const TIMEOUT_RATE_LIMIT: f64 = 0.001;
/// Early stop: at least this many timeouts...
const EARLY_STOP_MIN_TIMEOUTS: u32 = 10;
/// ...and a timeout rate above this multiple of the limit.
const EARLY_STOP_FACTOR: f64 = 10.0;
/// Early stop: this many samples that hit `max_attempts` (any failure makes a cell unsupported).
const EARLY_STOP_MIN_FAILED: usize = 10;

#[derive(Args, Debug, Clone)]
pub struct StatsArgs {
    #[arg(long, value_enum, default_value = "uniform")]
    pub generator: GeneratorKind,
    /// Range of `n_colors`, e.g. `2..=12`.
    #[arg(long, default_value = "2..=12")]
    pub colors: Range,
    /// Range of tube capacities.
    #[arg(long, default_value = "3..=5")]
    pub capacity: Range,
    /// Range of empty-tube counts.
    #[arg(long, default_value = "1..=3")]
    pub empty: Range,
    /// Puzzles per cell.
    #[arg(long, default_value_t = 1000)]
    pub samples: u32,
    /// Solver state-count limit (accepts `5e6`).
    #[arg(long, value_parser = parse_count, default_value = "5000000")]
    pub max_states: u64,
    #[arg(long, default_value_t = GenConfig::default().max_attempts)]
    pub max_attempts: u32,
    #[arg(long, default_value_t = GenConfig::default().min_opt)]
    pub min_opt: u32,
    /// Random rollouts per accepted puzzle (metrics only; never affects acceptance).
    #[arg(long, default_value_t = MetricsConfig::default().random_rollouts)]
    pub rollouts: u32,
    /// Sample `i` uses seed `splitmix64(base_seed ^ i)`.
    #[arg(long, default_value_t = 0)]
    pub base_seed: u64,
    /// Output path without extension; writes `<out>.csv` and `<out>.md`.
    #[arg(long, default_value = "reports/uniform_stats")]
    pub out: PathBuf,
    /// Worker threads. Each A* search can hold `max_states` states (hundreds of MB at 5e6),
    /// so this also bounds memory. Default: available cores, at most 8.
    #[arg(long)]
    pub threads: Option<usize>,
    /// Samples per parallel batch; early-stop checks run between batches.
    #[arg(long, default_value_t = 64)]
    pub batch: u32,
    /// Wall-clock budget per cell in seconds; a cell over budget is stopped and unsupported.
    #[arg(long, default_value_t = 900)]
    pub cell_budget_secs: u64,
    /// Keep measuring larger `n_colors` after a smaller cell (same capacity and `n_empty`) was
    /// stopped early.
    #[arg(long)]
    pub no_skip: bool,
}

impl StatsArgs {
    fn config(&self) -> GenConfig {
        GenConfig {
            min_opt: self.min_opt,
            max_attempts: self.max_attempts,
            max_states: self.max_states,
            metrics: MetricsConfig {
                random_rollouts: self.rollouts,
                ..MetricsConfig::default()
            },
        }
    }
}

/// One `generate` call.
#[derive(Debug, Default)]
struct Sample {
    /// `(opt_moves, symmetric)` if a puzzle was generated.
    puzzle: Option<(u32, bool)>,
    attempts: u32,
    rejections: RejectionCounts,
    /// Solver wall time of each attempt.
    solve_secs: Vec<f32>,
    /// Solver wall time of the accepted attempt.
    accepted_solve_secs: Option<f32>,
    /// States expanded by each attempt.
    states: Vec<u64>,
    /// Whole `generate` call, including metrics.
    gen_secs: f32,
}

/// Times the solver and counts rejections.
#[derive(Default)]
struct Probe {
    started: Option<Instant>,
    sample: Sample,
}

impl Observer for Probe {
    fn before_attempt(&mut self) {
        self.started = Some(Instant::now());
    }

    fn after_attempt(&mut self, evaluation: &Evaluation) {
        let secs = self
            .started
            .take()
            .map_or(0.0, |t| t.elapsed().as_secs_f32());
        let s = &mut self.sample;
        s.attempts += 1;
        s.solve_secs.push(secs);
        s.states.push(evaluation.states_expanded);
        match &evaluation.outcome {
            Ok(_) => s.accepted_solve_secs = Some(secs),
            Err(rejection) => s.rejections.record(rejection),
        }
    }
}

fn sample<G: Generator>(g: &G, params: Params, seed: u64, cfg: &GenConfig) -> Sample {
    let start = Instant::now();
    let mut probe = Probe::default();
    let result = g.generate_observed(&params, seed, cfg, &mut probe);
    let mut sample = probe.sample;
    sample.gen_secs = start.elapsed().as_secs_f32();
    sample.puzzle = result.ok().map(|p| (p.opt_moves, is_symmetric(&p.state)));
    sample
}

/// How a cell's measurement ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Complete,
    /// Stopped early: the timeout rate clearly exceeds the D3 limit.
    StoppedTimeouts,
    /// Stopped early: at least `EARLY_STOP_MIN_FAILED` samples hit `max_attempts`.
    StoppedFailures,
    /// Stopped early: the cell's time budget ran out.
    StoppedBudget,
    /// Not measured: a smaller `n_colors` cell with the same capacity and `n_empty` was stopped.
    Skipped,
    /// The params exceed the core limits.
    Invalid,
}

impl Status {
    const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::StoppedTimeouts => "stopped:timeouts",
            Self::StoppedFailures => "stopped:failures",
            Self::StoppedBudget => "stopped:budget",
            Self::Skipped => "skipped",
            Self::Invalid => "invalid",
        }
    }

    const fn stopped(self) -> bool {
        matches!(
            self,
            Self::StoppedTimeouts | Self::StoppedFailures | Self::StoppedBudget
        )
    }
}

/// Aggregated measurements of one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub params: Params,
    pub status: Status,
    pub samples: u32,
    /// Samples that hit `max_attempts`.
    pub failed: u32,
    pub attempts_total: u64,
    pub rejections: RejectionCounts,
    pub attempts_mean: f64,
    pub attempts_p50: u32,
    pub attempts_p99: u32,
    pub opt_mean: f64,
    pub opt_p50: u32,
    pub opt_p99: u32,
    /// `(opt_moves, count)`, ascending.
    pub opt_hist: Vec<(u32, u32)>,
    pub symmetric_frac: f64,
    pub states_p50: u64,
    pub states_p99: u64,
    /// Timing columns (not deterministic).
    pub solve_ms_p50: f64,
    pub solve_ms_p99: f64,
    pub accepted_solve_ms_p99: f64,
    pub gen_ms_p50: f64,
    pub gen_ms_p99: f64,
    pub wall_secs: f64,
}

impl Cell {
    fn empty(params: Params, status: Status) -> Self {
        Self {
            params,
            status,
            samples: 0,
            failed: 0,
            attempts_total: 0,
            rejections: RejectionCounts::default(),
            attempts_mean: f64::NAN,
            attempts_p50: 0,
            attempts_p99: 0,
            opt_mean: f64::NAN,
            opt_p50: 0,
            opt_p99: 0,
            opt_hist: Vec::new(),
            symmetric_frac: f64::NAN,
            states_p50: 0,
            states_p99: 0,
            solve_ms_p50: f64::NAN,
            solve_ms_p99: f64::NAN,
            accepted_solve_ms_p99: f64::NAN,
            gen_ms_p50: f64::NAN,
            gen_ms_p99: f64::NAN,
            wall_secs: 0.0,
        }
    }

    /// Fraction of attempts rejected for `count`.
    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    pub fn rate(&self, count: u32) -> f64 {
        if self.attempts_total == 0 {
            f64::NAN
        } else {
            f64::from(count) / self.attempts_total as f64
        }
    }

    pub fn timeout_rate(&self) -> f64 {
        self.rate(self.rejections.timeout)
    }

    /// The proposed D3 criterion: fully measured, no failed samples, solver p99 below 1 s both
    /// per attempt and over accepted puzzles, and timeout rate below 0.1 % per attempt.
    pub fn supported(&self) -> bool {
        self.status == Status::Complete
            && self.failed == 0
            && self.solve_ms_p99 < P99_LIMIT_MS
            && self.accepted_solve_ms_p99 < P99_LIMIT_MS
            && self.timeout_rate() < TIMEOUT_RATE_LIMIT
    }
}

/// Nearest-rank percentile of sorted values; `q` in `(0, 1]`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn percentile<T: Copy>(sorted: &[T], q: f64) -> Option<T> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

#[allow(clippy::cast_precision_loss)] // counts are far below 2^52
fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = values.fold((0.0, 0u64), |(s, n), v| (s + v, n + 1));
    if n == 0 { f64::NAN } else { sum / n as f64 }
}

fn summarize(params: Params, status: Status, samples: &[Sample], wall: Duration) -> Cell {
    let mut cell = Cell::empty(params, status);
    cell.samples = u32::try_from(samples.len()).expect("sample count fits in u32");
    cell.wall_secs = wall.as_secs_f64();
    if samples.is_empty() {
        return cell;
    }
    let mut attempts: Vec<u32> = samples.iter().map(|s| s.attempts).collect();
    let mut opts: Vec<u32> = samples
        .iter()
        .filter_map(|s| s.puzzle.map(|p| p.0))
        .collect();
    let mut expanded: Vec<u64> = samples
        .iter()
        .flat_map(|s| s.states.iter().copied())
        .collect();
    let mut solve: Vec<f32> = samples
        .iter()
        .flat_map(|s| s.solve_secs.iter().copied())
        .collect();
    let mut accepted: Vec<f32> = samples
        .iter()
        .filter_map(|s| s.accepted_solve_secs)
        .collect();
    let mut gen_secs: Vec<f32> = samples.iter().map(|s| s.gen_secs).collect();
    attempts.sort_unstable();
    opts.sort_unstable();
    expanded.sort_unstable();
    solve.sort_unstable_by(f32::total_cmp);
    accepted.sort_unstable_by(f32::total_cmp);
    gen_secs.sort_unstable_by(f32::total_cmp);

    cell.failed = cell.samples - u32::try_from(opts.len()).expect("fits in u32");
    cell.attempts_total = attempts.iter().map(|&a| u64::from(a)).sum();
    for s in samples {
        let r = &s.rejections;
        cell.rejections.already_solved += r.already_solved;
        cell.rejections.unsolvable += r.unsolvable;
        cell.rejections.timeout += r.timeout;
        cell.rejections.below_min_opt += r.below_min_opt;
    }
    cell.attempts_mean = mean(attempts.iter().map(|&a| f64::from(a)));
    cell.attempts_p50 = percentile(&attempts, 0.5).unwrap_or(0);
    cell.attempts_p99 = percentile(&attempts, 0.99).unwrap_or(0);
    cell.opt_mean = mean(opts.iter().map(|&o| f64::from(o)));
    cell.opt_p50 = percentile(&opts, 0.5).unwrap_or(0);
    cell.opt_p99 = percentile(&opts, 0.99).unwrap_or(0);
    for &o in &opts {
        match cell.opt_hist.last_mut() {
            Some((v, n)) if *v == o => *n += 1,
            _ => cell.opt_hist.push((o, 1)),
        }
    }
    cell.symmetric_frac = mean(
        samples
            .iter()
            .filter_map(|s| s.puzzle.map(|p| f64::from(u8::from(p.1)))),
    );
    cell.states_p50 = percentile(&expanded, 0.5).unwrap_or(0);
    cell.states_p99 = percentile(&expanded, 0.99).unwrap_or(0);
    let ms = |v: Option<f32>| v.map_or(f64::NAN, |s| f64::from(s) * 1000.0);
    cell.solve_ms_p50 = ms(percentile(&solve, 0.5));
    cell.solve_ms_p99 = ms(percentile(&solve, 0.99));
    cell.accepted_solve_ms_p99 = ms(percentile(&accepted, 0.99));
    cell.gen_ms_p50 = ms(percentile(&gen_secs, 0.5));
    cell.gen_ms_p99 = ms(percentile(&gen_secs, 0.99));
    cell
}

/// Measures one cell.
fn run_cell<G: Generator + Sync>(
    g: &G,
    params: Params,
    args: &StatsArgs,
    pool: &rayon::ThreadPool,
) -> Cell {
    let cfg = args.config();
    let start = Instant::now();
    let budget = Duration::from_secs(args.cell_budget_secs);
    let mut samples: Vec<Sample> = Vec::with_capacity(args.samples as usize);
    let mut status = Status::Complete;
    let mut next = 0;
    while next < args.samples {
        let end = next.saturating_add(args.batch.max(1)).min(args.samples);
        let batch: Vec<Sample> = pool.install(|| {
            (next..end)
                .into_par_iter()
                .map(|i| sample(g, params, splitmix64(args.base_seed ^ u64::from(i)), &cfg))
                .collect()
        });
        samples.extend(batch);
        next = end;
        let timeouts: u32 = samples.iter().map(|s| s.rejections.timeout).sum();
        let attempts: u64 = samples.iter().map(|s| u64::from(s.attempts)).sum();
        #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
        let clearly_over =
            f64::from(timeouts) > EARLY_STOP_FACTOR * TIMEOUT_RATE_LIMIT * attempts as f64;
        if next < args.samples && timeouts >= EARLY_STOP_MIN_TIMEOUTS && clearly_over {
            status = Status::StoppedTimeouts;
            break;
        }
        let failed = samples.iter().filter(|s| s.puzzle.is_none()).count();
        if next < args.samples && failed >= EARLY_STOP_MIN_FAILED {
            status = Status::StoppedFailures;
            break;
        }
        if next < args.samples && start.elapsed() > budget {
            status = Status::StoppedBudget;
            break;
        }
    }
    summarize(params, status, &samples, start.elapsed())
}

/// Runs the whole grid, rewriting the reports after every cell.
///
/// # Errors
///
/// Thread-pool or file errors.
pub fn run(args: &StatsArgs) -> Result<Vec<Cell>, Box<dyn std::error::Error>> {
    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    let mut cells = Vec::new();
    for capacity in args.capacity.0.clone() {
        for n_empty in args.empty.0.clone() {
            let mut stopped = false;
            for n_colors in args.colors.0.clone() {
                let params = Params {
                    n_colors,
                    capacity,
                    n_empty,
                };
                let cell = if params.validate().is_err() {
                    Cell::empty(params, Status::Invalid)
                } else if stopped && !args.no_skip {
                    Cell::empty(params, Status::Skipped)
                } else {
                    match args.generator {
                        GeneratorKind::Uniform => run_cell(&Uniform, params, args, &pool),
                    }
                };
                stopped |= cell.status.stopped();
                eprintln!(
                    "{:>2} colors, cap {}, {} empty: {:<16} {:>5} samples, timeout {:.4}%, solve p99 {:.1} ms, {:.1} s",
                    n_colors,
                    capacity,
                    n_empty,
                    cell.status.label(),
                    cell.samples,
                    cell.timeout_rate() * 100.0,
                    cell.solve_ms_p99,
                    cell.wall_secs
                );
                cells.push(cell);
                write_reports(&args.out, args, threads, &cells)?;
            }
        }
    }
    Ok(cells)
}

fn with_suffix(out: &Path, ext: &str) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

fn write_reports(
    out: &Path,
    args: &StatsArgs,
    threads: usize,
    cells: &[Cell],
) -> std::io::Result<()> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(with_suffix(out, "csv"), csv(args, cells))?;
    std::fs::write(with_suffix(out, "md"), markdown(args, threads, cells))
}

/// Formats a float, or an empty string for NaN.
fn f(v: f64, decimals: usize) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v:.decimals$}")
    }
}

fn hist(cell: &Cell) -> String {
    cell.opt_hist
        .iter()
        .map(|(o, n)| format!("{o}:{n}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub const CSV_HEADER: &str = "generator,n_colors,capacity,n_empty,status,supported,samples,failed,\
attempts_total,rate_unsolvable,rate_timeout,rate_below_min_opt,rate_already_solved,\
attempts_mean,attempts_p50,attempts_p99,opt_mean,opt_p50,opt_p99,symmetric_frac,\
states_p50,states_p99,solve_ms_p50,solve_ms_p99,accepted_solve_ms_p99,gen_ms_p50,gen_ms_p99,\
wall_secs,opt_hist";

fn csv(args: &StatsArgs, cells: &[Cell]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for c in cells {
        let r = &c.rejections;
        let fields = [
            args.generator.id().to_string(),
            c.params.n_colors.to_string(),
            c.params.capacity.to_string(),
            c.params.n_empty.to_string(),
            c.status.label().to_string(),
            c.supported().to_string(),
            c.samples.to_string(),
            c.failed.to_string(),
            c.attempts_total.to_string(),
            f(c.rate(r.unsolvable), 6),
            f(c.rate(r.timeout), 6),
            f(c.rate(r.below_min_opt), 6),
            f(c.rate(r.already_solved), 6),
            f(c.attempts_mean, 3),
            c.attempts_p50.to_string(),
            c.attempts_p99.to_string(),
            f(c.opt_mean, 3),
            c.opt_p50.to_string(),
            c.opt_p99.to_string(),
            f(c.symmetric_frac, 4),
            c.states_p50.to_string(),
            c.states_p99.to_string(),
            f(c.solve_ms_p50, 3),
            f(c.solve_ms_p99, 3),
            f(c.accepted_solve_ms_p99, 3),
            f(c.gen_ms_p50, 3),
            f(c.gen_ms_p99, 3),
            f(c.wall_secs, 1),
            hist(c),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

fn pct(v: f64) -> String {
    f(v * 100.0, 2)
}

fn markdown(args: &StatsArgs, threads: usize, cells: &[Cell]) -> String {
    let cfg = args.config();
    let mut md = String::new();
    let _ = writeln!(md, "# `{}` generator statistics\n", args.generator.id());
    let _ = writeln!(
        md,
        "Produced by `water_sort_cli stats --generator {} --colors {}..={} --capacity {}..={} \
         --empty {}..={} --samples {} --max-states {} --max-attempts {} --min-opt {} \
         --base-seed {}` (release build, {threads} threads, batch {}, cell budget {} s). \
         Sample `i` of every cell uses seed `splitmix64(base_seed ^ i)`.\n",
        args.generator.id(),
        args.colors.0.start(),
        args.colors.0.end(),
        args.capacity.0.start(),
        args.capacity.0.end(),
        args.empty.0.start(),
        args.empty.0.end(),
        args.samples,
        args.max_states,
        cfg.max_attempts,
        cfg.min_opt,
        args.base_seed,
        args.batch,
        args.cell_budget_secs,
    );
    let _ = writeln!(
        md,
        "Rates are per attempt. `solve` is the solver wall time of each attempt (all \
         outcomes); `acc. p99` only over accepted attempts; `states` is states expanded per \
         attempt. Timing columns depend on the machine and on parallel load; all other columns \
         are deterministic. `sym` is the fraction of generated puzzles with a nontrivial \
         symmetry (D2). Supported (proposed D3): complete, no failed samples (a sample fails when \
         it hits `max_attempts`), solve p99 and acc. p99 < {} ms, and timeout rate < {} %. A \
         cell stops early once it has at least {EARLY_STOP_MIN_TIMEOUTS} timeouts and a timeout \
         rate above {} % (`stopped:timeouts`), or {EARLY_STOP_MIN_FAILED} failed samples \
         (`stopped:failures`); larger `n_colors` with the same capacity and `n_empty` are then \
         skipped. Default solver limit: {} states.\n",
        P99_LIMIT_MS,
        TIMEOUT_RATE_LIMIT * 100.0,
        EARLY_STOP_FACTOR * TIMEOUT_RATE_LIMIT * 100.0,
        SolverLimits::DEFAULT_MAX_STATES,
    );
    md.push_str(
        "| colors | cap | empty | status | ok | samples | unsolv. % | timeout % | below min % \
         | attempts mean / p99 | opt mean / p50 / p99 | solve ms p50 / p99 | acc. p99 ms \
         | states p50 / p99 | sym % |\n",
    );
    md.push_str("|---:|---:|---:|---|:-:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for c in cells {
        let r = &c.rejections;
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {}{} | {} | {} | {} | {} / {} | {} / {} / {} | {} / {} \
             | {} | {} / {} | {} |",
            c.params.n_colors,
            c.params.capacity,
            c.params.n_empty,
            c.status.label(),
            if c.supported() { "yes" } else { "no" },
            c.samples,
            if c.failed > 0 {
                format!(" ({} failed)", c.failed)
            } else {
                String::new()
            },
            pct(c.rate(r.unsolvable)),
            pct(c.rate(r.timeout)),
            pct(c.rate(r.below_min_opt)),
            f(c.attempts_mean, 2),
            c.attempts_p99,
            f(c.opt_mean, 1),
            c.opt_p50,
            c.opt_p99,
            f(c.solve_ms_p50, 2),
            f(c.solve_ms_p99, 1),
            f(c.accepted_solve_ms_p99, 1),
            c.states_p50,
            c.states_p99,
            pct(c.symmetric_frac),
        );
    }
    md.push_str("\n## `opt_moves` histograms\n\n");
    for c in cells.iter().filter(|c| !c.opt_hist.is_empty()) {
        let _ = writeln!(
            md,
            "- {} colors, cap {}, {} empty: `{}`",
            c.params.n_colors,
            c.params.capacity,
            c.params.n_empty,
            hist(c)
        );
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: StatsArgs,
    }

    fn args(extra: &[&str]) -> StatsArgs {
        let mut argv = vec!["stats"];
        argv.extend_from_slice(extra);
        Wrapper::parse_from(argv).args
    }

    #[test]
    fn percentiles() {
        let v: Vec<u32> = (1..=100).collect();
        assert_eq!(percentile(&v, 0.5), Some(50));
        assert_eq!(percentile(&v, 0.99), Some(99));
        assert_eq!(percentile(&v, 1.0), Some(100));
        assert_eq!(percentile(&[7u32], 0.99), Some(7));
        assert_eq!(percentile::<u32>(&[], 0.5), None);
    }

    #[test]
    fn small_cells_are_deterministic() {
        let a = args(&[
            "--colors",
            "3..=4",
            "--capacity",
            "3",
            "--empty",
            "1",
            "--samples",
            "40",
            "--batch",
            "7",
            "--rollouts",
            "0",
        ]);
        let pool = |n| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .unwrap()
        };
        let p = Params {
            n_colors: 4,
            capacity: 3,
            n_empty: 1,
        };
        let one = run_cell(&Uniform, p, &a, &pool(1));
        let four = run_cell(&Uniform, p, &a, &pool(4));
        let strip = |c: &Cell| {
            let mut c = c.clone();
            for v in [
                &mut c.solve_ms_p50,
                &mut c.solve_ms_p99,
                &mut c.accepted_solve_ms_p99,
                &mut c.gen_ms_p50,
                &mut c.gen_ms_p99,
                &mut c.wall_secs,
            ] {
                *v = 0.0;
            }
            c
        };
        assert_eq!(strip(&one), strip(&four));
        assert_eq!(one.status, Status::Complete);
        assert_eq!((one.samples, one.failed), (40, 0));
        assert_eq!(one.attempts_total, 40 + u64::from(one.rejections.total()));
        assert!(one.rejections.unsolvable > 0, "n_empty = 1 rejects fills");
        assert_eq!(one.opt_hist.iter().map(|h| h.1).sum::<u32>(), 40);
        assert!(one.supported());
        // Report rows carry every cell.
        let cells = vec![one.clone(), Cell::empty(p, Status::Skipped)];
        let text = csv(&a, &cells);
        assert_eq!(text.lines().count(), 3);
        let header_fields = CSV_HEADER.split(',').count();
        assert!(text.lines().all(|l| l.split(',').count() == header_fields));
        let md = markdown(&a, 1, &cells);
        assert!(md.contains("| 4 | 3 | 1 | complete | yes | 40 |"));
        assert!(md.contains("| skipped | no |"));
    }

    #[test]
    fn early_stop_on_timeouts() {
        // A tiny state limit makes most solves time out.
        let a = args(&[
            "--samples",
            "200",
            "--batch",
            "16",
            "--max-states",
            "50",
            "--max-attempts",
            "20",
            "--rollouts",
            "0",
        ]);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let p = Params {
            n_colors: 5,
            capacity: 4,
            n_empty: 2,
        };
        let cell = run_cell(&Uniform, p, &a, &pool);
        assert_eq!(cell.status, Status::StoppedTimeouts);
        assert_eq!(cell.samples, 16);
        assert!(!cell.supported());
    }

    #[test]
    fn early_stop_on_failures() {
        // One attempt per sample: most n_empty = 1 fills are unsolvable, so most samples fail.
        let a = args(&[
            "--samples",
            "200",
            "--batch",
            "32",
            "--max-attempts",
            "1",
            "--rollouts",
            "0",
        ]);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let p = Params {
            n_colors: 6,
            capacity: 3,
            n_empty: 1,
        };
        let cell = run_cell(&Uniform, p, &a, &pool);
        assert_eq!(cell.status, Status::StoppedFailures);
        assert_eq!(cell.samples, 32);
        assert!(cell.failed >= 10 && !cell.supported());
    }

    #[test]
    fn grid_skips_after_a_stopped_cell() {
        let dir = std::env::temp_dir().join(format!("wsc_stats_{}", std::process::id()));
        let out = dir.join("grid");
        let out_arg = out.to_str().unwrap();
        let a = args(&[
            "--colors",
            "4..=6",
            "--capacity",
            "4",
            "--empty",
            "2",
            "--samples",
            "32",
            "--batch",
            "16",
            "--max-states",
            "50",
            "--max-attempts",
            "20",
            "--rollouts",
            "0",
            "--threads",
            "2",
            "--out",
            out_arg,
        ]);
        let cells = run(&a).unwrap();
        let statuses: Vec<Status> = cells.iter().map(|c| c.status).collect();
        assert_eq!(statuses[0], Status::StoppedTimeouts);
        assert_eq!(&statuses[1..], &[Status::Skipped, Status::Skipped]);
        assert!(with_suffix(&out, "csv").exists() && with_suffix(&out, "md").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod report_tests {
    use water_sort_core::{Params, is_supported};

    /// The core `SUPPORTED` table must match the committed report cell by cell.
    #[test]
    fn supported_table_matches_report() {
        let csv = include_str!("../../reports/uniform_stats.csv");
        let mut lines = csv.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        assert_eq!(header, super::CSV_HEADER.split(',').collect::<Vec<_>>());
        let col = |name: &str| header.iter().position(|h| *h == name).unwrap();
        let (c, k, e, s) = (
            col("n_colors"),
            col("capacity"),
            col("n_empty"),
            col("supported"),
        );
        let mut rows = 0;
        for line in lines {
            let fields: Vec<&str> = line.split(',').collect();
            let params = Params {
                n_colors: fields[c].parse().unwrap(),
                capacity: fields[k].parse().unwrap(),
                n_empty: fields[e].parse().unwrap(),
            };
            assert_eq!(is_supported(&params), fields[s] == "true", "{params:?}");
            rows += 1;
        }
        assert_eq!(rows, 99);
    }
}
