//! `water_sort_cli compare`: two generator specs side by side on the same configurations, with
//! histograms, two-sample tests and canonical-hash overlap (Phase 3).
//!
//! Both sides use the same seed list `splitmix64(base_seed ^ i)` and the same `GenConfig`, so
//! every non-timing number is deterministic.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use clap::Args;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use water_sort_core::{GenConfig, MetricsConfig, Params, Tier};

use crate::args::{GenSpec, parse_count};
use crate::stats::{Cell, CellRun, Generated, Sampling, collect_samples, summarize};

#[derive(Args, Debug, Clone)]
pub struct CompareArgs {
    /// First generator, e.g. `uniform`, `uniform:distributed`, `turan:scramble:40:standard`.
    #[arg(long)]
    pub a: GenSpec,
    /// Second generator.
    #[arg(long)]
    pub b: GenSpec,
    /// Configurations as `<colors>x<capacity>x<empty>`, comma-separated.
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2"
    )]
    pub configs: Vec<ParamsArg>,
    /// Puzzles per configuration and generator.
    #[arg(long, default_value_t = 1000)]
    pub samples: u32,
    #[arg(long, value_parser = parse_count, default_value = "5000000")]
    pub max_states: u64,
    #[arg(long, default_value_t = GenConfig::default().max_attempts)]
    pub max_attempts: u32,
    #[arg(long, default_value_t = GenConfig::default().min_opt)]
    pub min_opt: u32,
    /// Reject puzzles with `opt_moves` above this (D16).
    #[arg(long)]
    pub max_opt: Option<u32>,
    /// Compare within one difficulty tier (D16): each side's `opt_moves` band per configuration
    /// from the core tier table, for that side's layout.
    #[arg(long, conflicts_with_all = ["min_opt", "max_opt"])]
    pub tier: Option<Tier>,
    #[arg(long, default_value_t = MetricsConfig::default().random_rollouts)]
    pub rollouts: u32,
    #[arg(long, default_value_t = 0)]
    pub base_seed: u64,
    /// Output path without extension; writes `<out>.md`. Default: `reports/<a>_vs_<b>`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub threads: Option<usize>,
}

/// `n_colors x capacity x n_empty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamsArg(pub Params);

impl core::str::FromStr for ParamsArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.trim().split('x').collect();
        let [c, k, e] = parts.as_slice() else {
            return Err(format!("expected <colors>x<capacity>x<empty>, got {s:?}"));
        };
        let num = |t: &str| t.parse::<u8>().map_err(|e| format!("{s:?}: {e}"));
        let params = Params {
            n_colors: num(c)?,
            capacity: num(k)?,
            n_empty: num(e)?,
        };
        params.validate().map_err(|e| format!("{s:?}: {e}"))?;
        Ok(Self(params))
    }
}

impl CompareArgs {
    fn config(&self) -> GenConfig {
        GenConfig {
            min_opt: self.min_opt,
            max_opt: self.max_opt,
            max_attempts: self.max_attempts,
            max_states: self.max_states,
            metrics: MetricsConfig {
                random_rollouts: self.rollouts,
                ..MetricsConfig::default()
            },
        }
    }

    fn out_path(&self) -> PathBuf {
        self.out.clone().unwrap_or_else(|| {
            PathBuf::from("reports").join(format!("{}_vs_{}", self.a.slug(), self.b.slug()))
        })
    }
}

/// One side of one configuration.
struct Side {
    cell: Cell,
    puzzles: Vec<Generated>,
}

fn run_side(spec: GenSpec, params: Params, args: &CompareArgs, pool: &rayon::ThreadPool) -> Side {
    let mut cfg = args.config();
    if let Some(t) = args.tier {
        // `run` checked every configuration against the tier table first.
        let (min_opt, max_opt) = t
            .opt_band(&params, spec.layout())
            .expect("supported configuration");
        cfg.min_opt = min_opt;
        cfg.max_opt = max_opt;
    }
    let sampling = Sampling {
        samples: args.samples,
        batch: 64,
        base_seed: args.base_seed,
        cell_budget_secs: u64::MAX / 2,
    };
    let run: CellRun = match spec {
        GenSpec::Uniform(g) => collect_samples(&g, params, &cfg, &sampling, pool),
        GenSpec::Turan(g) => collect_samples(&g, params, &cfg, &sampling, pool),
    };
    let cell = summarize(params, run.status, cfg.max_attempts, &run.samples, run.wall);
    let puzzles = run.samples.iter().filter_map(|s| s.puzzle).collect();
    Side { cell, puzzles }
}

/// Runs the comparison and writes the report.
///
/// # Errors
///
/// Thread-pool or file errors.
pub fn run(args: &CompareArgs) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    if let Some(t) = args.tier {
        for &ParamsArg(p) in &args.configs {
            for spec in [args.a, args.b] {
                if t.opt_band(&p, spec.layout()).is_none() {
                    return Err(format!(
                        "--tier: {}x{}x{} is not supported in the {} layout",
                        p.n_colors,
                        p.capacity,
                        p.n_empty,
                        spec.layout()
                    )
                    .into());
                }
            }
        }
    }
    let mut sections = Vec::new();
    for &ParamsArg(params) in &args.configs {
        let a = run_side(args.a, params, args, &pool);
        let b = run_side(args.b, params, args, &pool);
        eprintln!(
            "{}x{}x{}: opt mean {:.1} vs {:.1}, {:.1} s + {:.1} s",
            params.n_colors,
            params.capacity,
            params.n_empty,
            a.cell.opt_mean,
            b.cell.opt_mean,
            a.cell.wall_secs,
            b.cell.wall_secs
        );
        sections.push(section(params, &a, &b));
    }
    let out = args.out_path();
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut path = out.into_os_string();
    path.push(".md");
    let path = PathBuf::from(path);
    std::fs::write(&path, report(args, threads, &sections))?;
    Ok(path)
}

/// A two-sample test result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestResult {
    pub statistic: f64,
    /// Degrees of freedom (chi-square) or effective sample size (KS).
    pub df: f64,
    pub p_value: f64,
}

/// Two-sample Pearson chi-square on histograms (`value -> count`). Adjacent bins (in value
/// order) are merged until each holds at least `MIN_BIN` observations over both samples.
pub fn chi_square_two_sample(a: &BTreeMap<u32, u64>, b: &BTreeMap<u32, u64>) -> TestResult {
    const MIN_BIN: u64 = 10;
    let keys: std::collections::BTreeSet<u32> = a.keys().chain(b.keys()).copied().collect();
    let mut bins: Vec<(u64, u64)> = Vec::new();
    let mut acc = (0, 0);
    for k in keys {
        acc.0 += a.get(&k).copied().unwrap_or(0);
        acc.1 += b.get(&k).copied().unwrap_or(0);
        if acc.0 + acc.1 >= MIN_BIN {
            bins.push(acc);
            acc = (0, 0);
        }
    }
    if acc.0 + acc.1 > 0 {
        match bins.last_mut() {
            Some(last) => {
                last.0 += acc.0;
                last.1 += acc.1;
            }
            None => bins.push(acc),
        }
    }
    let na: u64 = bins.iter().map(|x| x.0).sum();
    let nb: u64 = bins.iter().map(|x| x.1).sum();
    if bins.len() < 2 || na == 0 || nb == 0 {
        return TestResult {
            statistic: 0.0,
            df: 0.0,
            p_value: 1.0,
        };
    }
    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    let (na, nb) = (na as f64, nb as f64);
    let (ka, kb) = ((nb / na).sqrt(), (na / nb).sqrt());
    #[allow(clippy::cast_precision_loss)]
    let statistic: f64 = bins
        .iter()
        .map(|&(x, y)| {
            let (x, y) = (x as f64, y as f64);
            (ka * x - kb * y).powi(2) / (x + y)
        })
        .sum();
    #[allow(clippy::cast_precision_loss)]
    let df = (bins.len() - 1) as f64;
    TestResult {
        statistic,
        df,
        p_value: ChiSquared::new(df).expect("df > 0").sf(statistic),
    }
}

/// Two-sample Kolmogorov-Smirnov test with the asymptotic p-value. With ties (discrete data) the
/// test is conservative: the reported p-value is an upper bound.
pub fn ks_two_sample(xs: &[f64], ys: &[f64]) -> TestResult {
    if xs.is_empty() || ys.is_empty() {
        return TestResult {
            statistic: 0.0,
            df: 0.0,
            p_value: 1.0,
        };
    }
    let mut xs = xs.to_vec();
    let mut ys = ys.to_vec();
    xs.sort_unstable_by(f64::total_cmp);
    ys.sort_unstable_by(f64::total_cmp);
    #[allow(clippy::cast_precision_loss)]
    let (nx, ny) = (xs.len() as f64, ys.len() as f64);
    let (mut ix, mut iy, mut gap) = (0, 0, 0.0f64);
    while ix < xs.len() && iy < ys.len() {
        let at = xs[ix].min(ys[iy]);
        while ix < xs.len() && xs[ix] <= at {
            ix += 1;
        }
        while iy < ys.len() && ys[iy] <= at {
            iy += 1;
        }
        #[allow(clippy::cast_precision_loss)]
        let here = (ix as f64 / nx - iy as f64 / ny).abs();
        gap = gap.max(here);
    }
    let ne = nx * ny / (nx + ny);
    let lambda = (ne.sqrt() + 0.12 + 0.11 / ne.sqrt()) * gap;
    TestResult {
        statistic: gap,
        df: ne,
        p_value: kolmogorov_sf(lambda),
    }
}

/// `P(K > lambda)` for the Kolmogorov distribution.
fn kolmogorov_sf(lambda: f64) -> f64 {
    if lambda < 1e-3 {
        return 1.0;
    }
    let mut sum = 0.0;
    for k in 1..=100 {
        let k = f64::from(k);
        let term = 2.0 * (-1f64).powf(k - 1.0) * (-2.0 * k * k * lambda * lambda).exp();
        sum += term;
        if term.abs() < 1e-12 {
            break;
        }
    }
    sum.clamp(0.0, 1.0)
}

fn histogram(values: impl Iterator<Item = u32>) -> BTreeMap<u32, u64> {
    let mut h = BTreeMap::new();
    for v in values {
        *h.entry(v).or_insert(0) += 1;
    }
    h
}

#[allow(clippy::cast_precision_loss)]
fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        f64::NAN
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn f(v: f64, decimals: usize) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v:.decimals$}")
    }
}

fn p_value(p: f64) -> String {
    if p < 1e-300 {
        "< 1e-300".into()
    } else if p < 1e-3 {
        format!("{p:.1e}")
    } else {
        format!("{p:.3}")
    }
}

/// Per-puzzle metric accessors.
type Metric = (&'static str, fn(&Generated) -> f64);

#[allow(clippy::cast_precision_loss)]
const METRICS: [Metric; 6] = [
    ("opt_moves", |g| f64::from(g.opt_moves)),
    ("color changes", |g| f64::from(g.metrics.color_changes)),
    ("segments", |g| f64::from(g.metrics.segments)),
    ("random stuck rate", |g| {
        f64::from(g.metrics.random_stuck_rate)
    }),
    ("random capped rate", |g| {
        f64::from(g.metrics.random_capped_rate)
    }),
    ("states_expanded", |g| g.metrics.states_expanded as f64),
];

/// Integer-valued metrics that also get a histogram and a chi-square test.
type IntMetric = (&'static str, fn(&Generated) -> u32);

const HISTOGRAMS: [IntMetric; 3] = [
    ("opt_moves", |g| g.opt_moves),
    ("color changes", |g| g.metrics.color_changes),
    ("segments", |g| g.metrics.segments),
];

fn section(params: Params, a: &Side, b: &Side) -> String {
    let mut md = String::new();
    let _ = writeln!(
        md,
        "## {} colors, capacity {}, {} empty\n",
        params.n_colors, params.capacity, params.n_empty
    );
    md.push_str("| | A | B |\n|---|---:|---:|\n");
    let rows = |c: &Cell| -> Vec<String> {
        let r = &c.rejections;
        vec![
            format!(
                "{}{}",
                c.samples,
                if c.failed > 0 {
                    format!(" ({} failed)", c.failed)
                } else {
                    String::new()
                }
            ),
            if c.supported() {
                "yes".into()
            } else {
                "no".into()
            },
            format!("{} / {}", f(c.attempts_mean, 2), c.attempts_p99),
            f(c.rate(r.construction) * 100.0, 2),
            f(c.rate(r.unsolvable) * 100.0, 2),
            f(c.rate(r.timeout) * 100.0, 2),
            f(
                c.rate(r.below_min_opt + r.above_max_opt + r.already_solved) * 100.0,
                2,
            ),
            format!("{} / {} / {}", f(c.opt_mean, 2), c.opt_p50, c.opt_p99),
            format!("{} / {}", f(c.solve_ms_p50, 2), f(c.solve_ms_p99, 1)),
            format!("{} / {}", f(c.gen_ms_p50, 1), f(c.gen_ms_p99, 1)),
            format!("{} / {}", c.states_p50, c.states_p99),
            f(c.symmetric_frac * 100.0, 2),
        ]
    };
    let labels = [
        "samples",
        "D3 criterion met",
        "attempts mean / p99",
        "construction rejections %",
        "unsolvable %",
        "timeout %",
        "solved or outside opt band %",
        "opt mean / p50 / p99",
        "solve ms p50 / p99",
        "gen ms p50 / p99",
        "states/attempt p50 / p99",
        "symmetric %",
    ];
    for (label, (x, y)) in labels
        .iter()
        .zip(rows(&a.cell).into_iter().zip(rows(&b.cell)))
    {
        let _ = writeln!(md, "| {label} | {x} | {y} |");
    }
    tests_table(&mut md, a, b);
    overlap(&mut md, a, b);
    histograms(&mut md, a, b);
    md
}

fn tests_table(md: &mut String, a: &Side, b: &Side) {
    md.push_str("\n| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |\n");
    md.push_str("|---|---:|---:|---:|---:|---:|---:|\n");
    for (name, get) in METRICS {
        let xa: Vec<f64> = a.puzzles.iter().map(get).collect();
        let xb: Vec<f64> = b.puzzles.iter().map(get).collect();
        let ks = ks_two_sample(&xa, &xb);
        let chi = HISTOGRAMS.iter().find(|h| h.0 == name).map(|(_, h)| {
            chi_square_two_sample(
                &histogram(a.puzzles.iter().map(h)),
                &histogram(b.puzzles.iter().map(h)),
            )
        });
        let _ = writeln!(
            md,
            "| {name} | {} | {} | {} | {} | {} | {} |",
            f(mean(&xa), 3),
            f(mean(&xb), 3),
            chi.map_or(String::new(), |c| format!(
                "{} ({})",
                f(c.statistic, 1),
                c.df
            )),
            chi.map_or(String::new(), |c| p_value(c.p_value)),
            f(ks.statistic, 3),
            p_value(ks.p_value),
        );
    }
}

fn overlap(md: &mut String, a: &Side, b: &Side) {
    let ha: HashSet<u64> = a.puzzles.iter().map(|g| g.canonical_hash).collect();
    let hb: HashSet<u64> = b.puzzles.iter().map(|g| g.canonical_hash).collect();
    let shared = ha.intersection(&hb).count();
    let n = a.puzzles.len().min(b.puzzles.len());
    #[allow(clippy::cast_precision_loss)]
    let frac = |k: usize, n: usize| {
        if n == 0 {
            f64::NAN
        } else {
            k as f64 / n as f64
        }
    };
    let _ = writeln!(
        md,
        "\nCanonical-hash overlap: {shared} puzzles of A's {} distinct also occur in B's {} distinct \
         ({} % of {n}). Distinct fraction within each sample: A {} %, B {} %.\n",
        ha.len(),
        hb.len(),
        f(frac(shared, n) * 100.0, 2),
        f(frac(ha.len(), a.puzzles.len()) * 100.0, 2),
        f(frac(hb.len(), b.puzzles.len()) * 100.0, 2),
    );
}

fn histograms(md: &mut String, a: &Side, b: &Side) {
    for (name, get) in HISTOGRAMS {
        let h_a = histogram(a.puzzles.iter().map(get));
        let h_b = histogram(b.puzzles.iter().map(get));
        let keys: std::collections::BTreeSet<u32> = h_a.keys().chain(h_b.keys()).copied().collect();
        let cells: Vec<String> = keys
            .iter()
            .map(|k| {
                format!(
                    "{k}:{}/{}",
                    h_a.get(k).unwrap_or(&0),
                    h_b.get(k).unwrap_or(&0)
                )
            })
            .collect();
        let _ = writeln!(md, "- {name} histogram (value:A/B): `{}`", cells.join(" "));
    }
    for (name, get) in [METRICS[3], METRICS[4], METRICS[5]] {
        let q = |side: &Side| {
            let mut v: Vec<f64> = side.puzzles.iter().map(get).collect();
            v.sort_unstable_by(f64::total_cmp);
            [0.1, 0.5, 0.9, 0.99]
                .iter()
                .map(|&p| quantile(&v, p))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let _ = writeln!(
            md,
            "- {name} p10 / p50 / p90 / p99: A `{}`, B `{}`",
            q(a),
            q(b)
        );
    }
    md.push('\n');
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn quantile(sorted: &[f64], q: f64) -> String {
    if sorted.is_empty() {
        return String::new();
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    let v = sorted[rank.clamp(1, sorted.len()) - 1];
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.3}")
    }
}

fn report(args: &CompareArgs, threads: usize, sections: &[String]) -> String {
    let cfg = args.config();
    let mut md = String::new();
    let _ = writeln!(
        md,
        "# `{}` vs `{}`\n\n- A: `{}` / `{}`\n- B: `{}` / `{}`\n",
        args.a.slug(),
        args.b.slug(),
        args.a.id(),
        args.a.variant(),
        args.b.id(),
        args.b.variant()
    );
    let configs: Vec<String> = args
        .configs
        .iter()
        .map(|c| format!("{}x{}x{}", c.0.n_colors, c.0.capacity, c.0.n_empty))
        .collect();
    let _ = writeln!(
        md,
        "Produced by `water_sort_cli compare --a {} --b {} --configs {} --samples {} --max-states {} \
         --max-attempts {} --min-opt {}{} --rollouts {} --base-seed {}` (release build, {threads} \
         threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the \
         `ms` columns is deterministic.\n",
        spec_arg(args.a),
        spec_arg(args.b),
        configs.join(","),
        args.samples,
        cfg.max_states,
        cfg.max_attempts,
        cfg.min_opt,
        args.tier.map_or_else(
            || {
                cfg.max_opt
                    .map_or_else(String::new, |m| format!(" --max-opt {m}"))
            },
            |t| format!(" --tier {t}"),
        ),
        cfg.metrics.random_rollouts,
        args.base_seed,
    );
    md.push_str(
        "Per configuration: the stats measurements side by side (rates per attempt; \
         construction rejections are Turan attempts rejected before solving, D15), then per-puzzle \
         metric means with two-sample tests over the accepted puzzles. chi² is Pearson's \
         two-sample test on the histogram, with adjacent values merged until every bin holds \
         at least 10 puzzles; KS is the two-sample Kolmogorov-Smirnov test (asymptotic p-value; \
         conservative for discrete metrics). The random rates are per puzzle over its random \
         legal-move rollouts (stuck: no legal move left; capped: hit `4 × opt_moves` moves). \
         Overlap counts canonical hashes (puzzles equal up to tube order and color names) \
         present in both samples.\n\n",
    );
    for s in sections {
        md.push_str(s);
    }
    md
}

/// The `--a`/`--b` value that reproduces `spec`.
fn spec_arg(spec: GenSpec) -> String {
    use turan_water_sort::TuranStrategy;
    match spec {
        GenSpec::Uniform(g) => format!("uniform:{}", g.layout),
        GenSpec::Turan(g) => match g.strategy {
            TuranStrategy::Scramble {
                steps,
                max_extra_steps,
            } => format!(
                "turan:scramble:{steps}:extra={max_extra_steps}:{}",
                g.layout
            ),
            TuranStrategy::Constrained => format!("turan:constrained:{}", g.layout),
            TuranStrategy::PourWalk { steps } => format!("turan:walk:{steps}:{}", g.layout),
            TuranStrategy::ReverseSearch {
                max_depth,
                max_states,
            } => format!("turan:search:{max_states}:depth={max_depth}:{}", g.layout),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use water_sort_core::Layout;

    #[test]
    fn chi_square_detects_shift_and_accepts_equal() {
        let a = histogram([3, 4, 4, 5, 5, 5, 6, 6, 7].repeat(20).into_iter());
        let same = chi_square_two_sample(&a, &a);
        assert!(same.statistic.abs() < 1e-9 && (same.p_value - 1.0).abs() < 1e-9);
        let b = histogram([4, 5, 5, 6, 6, 6, 7, 7, 8].repeat(20).into_iter());
        let shifted = chi_square_two_sample(&a, &b);
        assert!(shifted.p_value < 1e-6, "{shifted:?}");
        // Unequal sizes with the same shape.
        let c = histogram([3, 4, 4, 5, 5, 5, 6, 6, 7].repeat(40).into_iter());
        assert!(chi_square_two_sample(&a, &c).p_value > 0.99);
        let empty = BTreeMap::new();
        assert!((chi_square_two_sample(&a, &empty).p_value - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ks_reference_values() {
        let a: Vec<f64> = (0..100).map(f64::from).collect();
        let same = ks_two_sample(&a, &a);
        assert!(same.statistic.abs() < 1e-12 && same.p_value > 0.99);
        let b: Vec<f64> = (50..150).map(f64::from).collect();
        let shifted = ks_two_sample(&a, &b);
        assert!((shifted.statistic - 0.5).abs() < 1e-12);
        assert!(shifted.p_value < 1e-9);
        // P(K > 1.36) is about 0.049 (the classic 5 % critical value).
        assert!((kolmogorov_sf(1.36) - 0.0494).abs() < 1e-3);
    }

    #[test]
    fn params_arg() {
        assert_eq!(
            "8x4x2".parse(),
            Ok(ParamsArg(Params {
                n_colors: 8,
                capacity: 4,
                n_empty: 2
            }))
        );
        assert!("8x4".parse::<ParamsArg>().is_err());
        assert!("20x4x2".parse::<ParamsArg>().is_err());
    }

    #[test]
    fn small_comparison_report() {
        let dir = std::env::temp_dir().join(format!("wsc_compare_{}", std::process::id()));
        let out = dir.join("cmp");
        let args = CompareArgs {
            a: "uniform".parse().unwrap(),
            b: "turan:scramble".parse().unwrap(),
            configs: vec!["4x3x2".parse().unwrap()],
            samples: 40,
            max_states: 5_000_000,
            max_attempts: 10_000,
            min_opt: 1,
            max_opt: None,
            tier: None,
            rollouts: 8,
            base_seed: 1,
            out: Some(out),
            threads: Some(2),
        };
        let path = run(&args).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("## 4 colors, capacity 3, 2 empty"));
        assert!(text.contains("| opt_moves |"));
        assert!(text.contains("--a uniform:standard --b turan:scramble:40:extra=100:standard"));
        // Within one tier: both sides only produce puzzles in the tier's band.
        let tiered = CompareArgs {
            b: "turan:search:500:distributed".parse().unwrap(),
            tier: Some(Tier::Medium),
            samples: 20,
            out: Some(dir.join("tier")),
            ..args.clone()
        };
        let text = std::fs::read_to_string(run(&tiered).unwrap()).unwrap();
        assert!(text.contains("--tier medium"));
        let p = tiered.configs[0].0;
        let hist = text
            .lines()
            .find_map(|l| l.strip_prefix("- opt_moves histogram (value:A/B): `"))
            .unwrap();
        for tok in hist.trim_end_matches('`').split(' ') {
            let (v, ab) = tok.split_once(':').unwrap();
            let (a, b) = ab.split_once('/').unwrap();
            let v: u32 = v.parse().unwrap();
            if a != "0" {
                assert_eq!(Tier::of(&p, Layout::Standard, v), Some(Tier::Medium));
            }
            if b != "0" {
                assert_eq!(Tier::of(&p, Layout::Distributed, v), Some(Tier::Medium));
            }
        }
        // A configuration outside the supported range is refused before any work.
        let unsupported = CompareArgs {
            configs: vec!["4x3x3".parse().unwrap()],
            ..tiered
        };
        assert!(run(&unsupported).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
