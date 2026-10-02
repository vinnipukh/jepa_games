//! `water_sort_cli sweep`: Turan `steps` sweep per layout (Phase 3 task 9, D16).
//!
//! For each `(layout, params, steps)`: the generation measurements (`opt_moves`, attempts,
//! rejections, timing) over `samples` puzzles. For the scramble, also the cost of the
//! construction itself over `samples` scramble candidates: how often the walk is absorbed in a
//! state without reverse moves, how often the standard layout is not reached, and the extra-step
//! distribution. `--strategy pour-walk` sweeps the walk's steps; `--strategy reverse-search`
//! sweeps the search's state budget (`--steps` values are then budgets).

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::Args;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use turan_water_sort::{Turan, TuranStrategy, scramble};
use water_sort_core::{
    GenConfig, Layout, MetricsConfig, Params, SUPPORTED, splitmix64, supported_rows,
};

use crate::args::{LayoutArg, StrategyArg};
use crate::compare::ParamsArg;
use crate::stats::{Cell, Sampling, collect_samples, summarize};

#[derive(Args, Debug, Clone)]
pub struct SweepArgs {
    /// Turan strategy: `scramble` (default), `pour-walk` (distributed only) or `reverse-search`.
    #[arg(long, value_enum, default_value = "scramble")]
    pub strategy: StrategyArg,
    /// Values of `steps` (reverse search: of its state budget).
    #[arg(long, value_delimiter = ',', default_value = "10,20,40,80,160")]
    pub steps: Vec<u32>,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "standard,distributed"
    )]
    pub layouts: Vec<LayoutArg>,
    /// Configurations (`<colors>x<capacity>x<empty>`); default: every supported one (D3).
    #[arg(long, value_delimiter = ',')]
    pub configs: Vec<ParamsArg>,
    /// Puzzles (and, separately, scramble candidates) per cell.
    #[arg(long, default_value_t = 300)]
    pub samples: u32,
    #[arg(long, default_value_t = TuranStrategy::DEFAULT_MAX_EXTRA_STEPS)]
    pub max_extra_steps: u32,
    /// Random rollouts per accepted puzzle (metrics only).
    #[arg(long, default_value_t = 0)]
    pub rollouts: u32,
    #[arg(long, default_value_t = 0)]
    pub base_seed: u64,
    /// Reverse search: path depth limit.
    #[arg(long, default_value_t = TuranStrategy::DEFAULT_SEARCH_DEPTH)]
    pub search_depth: u32,
    /// Output path without extension; writes `<out>.csv` and `<out>.md`. Default:
    /// `reports/turan_steps_sweep` (scramble), `reports/turan_pour_walk_steps_sweep`,
    /// `reports/turan_reverse_search_budget_sweep`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub threads: Option<usize>,
}

impl SweepArgs {
    fn out_path(&self) -> PathBuf {
        self.out.clone().unwrap_or_else(|| {
            PathBuf::from(match self.strategy {
                StrategyArg::PourWalk => "reports/turan_pour_walk_steps_sweep",
                StrategyArg::ReverseSearch => "reports/turan_reverse_search_budget_sweep",
                _ => "reports/turan_steps_sweep",
            })
        })
    }

    /// The strategy for one swept value.
    fn strategy(&self, steps: u32) -> Result<TuranStrategy, String> {
        match self.strategy {
            StrategyArg::Scramble => Ok(TuranStrategy::Scramble {
                steps,
                max_extra_steps: self.max_extra_steps,
            }),
            StrategyArg::PourWalk => Ok(TuranStrategy::PourWalk { steps }),
            StrategyArg::ReverseSearch => Ok(TuranStrategy::ReverseSearch {
                max_depth: self.search_depth,
                max_states: steps,
            }),
            StrategyArg::Constrained => Err("constrained has no steps to sweep".into()),
        }
    }

    /// The configurations for `layout`: `--configs`, else every supported one. The scramble
    /// sweep uses the standard table for both layouts (as in the committed Phase 3 report); the
    /// other strategies use the layout's own table (D3 and its distributed addendum).
    fn configs(&self, layout: Layout) -> Vec<Params> {
        if !self.configs.is_empty() {
            return self.configs.iter().map(|c| c.0).collect();
        }
        let rows = if self.strategy == StrategyArg::Scramble {
            SUPPORTED
        } else {
            supported_rows(layout)
        };
        rows.iter()
            .flat_map(|r| {
                (r.min_colors..=r.max_colors).map(move |n_colors| Params {
                    n_colors,
                    capacity: r.capacity,
                    n_empty: r.n_empty,
                })
            })
            .collect()
    }

    fn config(&self) -> GenConfig {
        GenConfig {
            metrics: MetricsConfig {
                random_rollouts: self.rollouts,
                ..MetricsConfig::default()
            },
            ..GenConfig::default()
        }
    }
}

/// Construction cost over scramble candidates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WalkStats {
    pub candidates: u32,
    /// Walks absorbed in a state without reverse moves.
    pub dead_ends: u32,
    /// Standard layout only: walks that did not reach it.
    pub not_returned: u32,
    /// Extra steps of the walks that reached the layout: p50, p90, p99, max.
    pub extra: [u32; 4],
}

fn walk_stats(params: Params, steps: u32, args: &SweepArgs, layout: Layout) -> WalkStats {
    if args.strategy != StrategyArg::Scramble {
        return WalkStats::default();
    }
    let mut out = WalkStats {
        candidates: args.samples,
        ..WalkStats::default()
    };
    let mut extra = Vec::new();
    for i in 0..args.samples {
        let mut rng = ChaCha20Rng::seed_from_u64(splitmix64(args.base_seed ^ u64::from(i)));
        let o = scramble(&mut rng, params, steps, args.max_extra_steps, layout);
        out.dead_ends += u32::from(o.dead_end);
        match o.state {
            Some(_) => extra.push(o.extra_steps),
            None => out.not_returned += 1,
        }
    }
    extra.sort_unstable();
    if !extra.is_empty() {
        let q = |p: f64| {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let rank = (p * extra.len() as f64).ceil() as usize;
            extra[rank.clamp(1, extra.len()) - 1]
        };
        out.extra = [q(0.5), q(0.9), q(0.99), extra[extra.len() - 1]];
    }
    out
}

struct Row {
    layout: Layout,
    steps: u32,
    cell: Cell,
    walk: WalkStats,
}

/// Runs the sweep and writes the reports.
///
/// # Errors
///
/// Thread-pool or file errors.
pub fn run(args: &SweepArgs) -> Result<(), Box<dyn std::error::Error>> {
    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    let cfg = args.config();
    let sampling = Sampling {
        samples: args.samples,
        batch: 64,
        base_seed: args.base_seed,
        cell_budget_secs: 900,
    };
    // Fail on an unsweepable strategy before any work.
    args.strategy(0)?;
    if args.strategy == StrategyArg::PourWalk
        && args
            .layouts
            .iter()
            .any(|&l| Layout::from(l) != Layout::Distributed)
    {
        return Err("pour-walk is distributed only: pass --layouts distributed".into());
    }
    let mut rows = Vec::new();
    for &layout in &args.layouts {
        let layout = Layout::from(layout);
        for params in args.configs(layout) {
            for &steps in &args.steps {
                let g = Turan::new(args.strategy(steps)?, layout);
                let run = collect_samples(&g, params, &cfg, &sampling, &pool);
                let cell = summarize(params, run.status, cfg.max_attempts, &run.samples, run.wall);
                let walk = walk_stats(params, steps, args, layout);
                eprintln!(
                    "{layout} {}x{}x{} steps {steps}: opt mean {:.2}, attempts mean {:.2}, {:.1} s",
                    params.n_colors,
                    params.capacity,
                    params.n_empty,
                    cell.opt_mean,
                    cell.attempts_mean,
                    cell.wall_secs
                );
                rows.push(Row {
                    layout,
                    steps,
                    cell,
                    walk,
                });
            }
            write(args, threads, &rows)?;
        }
    }
    Ok(())
}

fn write(args: &SweepArgs, threads: usize, rows: &[Row]) -> std::io::Result<()> {
    let out = args.out_path();
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let with = |ext: &str| {
        let mut s = out.clone().into_os_string();
        s.push(".");
        s.push(ext);
        PathBuf::from(s)
    };
    std::fs::write(with("csv"), csv(rows))?;
    std::fs::write(with("md"), markdown(args, threads, rows))
}

const CSV_HEADER: &str = "layout,n_colors,capacity,n_empty,steps,status,supported,samples,failed,\
attempts_mean,attempts_p99,rate_construction,rate_already_solved,rate_below_min_opt,rate_timeout,\
opt_mean,opt_p50,opt_p99,gen_ms_p50,gen_ms_p99,candidates,dead_ends,not_returned,extra_p50,\
extra_p90,extra_p99,extra_max";

fn csv(rows: &[Row]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for r in rows {
        let c = &r.cell;
        let j = &c.rejections;
        let w = &r.walk;
        let fields = [
            r.layout.to_string(),
            c.params.n_colors.to_string(),
            c.params.capacity.to_string(),
            c.params.n_empty.to_string(),
            r.steps.to_string(),
            format!("{:?}", c.status),
            c.supported().to_string(),
            c.samples.to_string(),
            c.failed.to_string(),
            format!("{:.3}", c.attempts_mean),
            c.attempts_p99.to_string(),
            format!("{:.6}", c.rate(j.construction)),
            format!("{:.6}", c.rate(j.already_solved)),
            format!("{:.6}", c.rate(j.below_min_opt)),
            format!("{:.6}", c.rate(j.timeout)),
            format!("{:.3}", c.opt_mean),
            c.opt_p50.to_string(),
            c.opt_p99.to_string(),
            format!("{:.3}", c.gen_ms_p50),
            format!("{:.3}", c.gen_ms_p99),
            w.candidates.to_string(),
            w.dead_ends.to_string(),
            w.not_returned.to_string(),
            w.extra[0].to_string(),
            w.extra[1].to_string(),
            w.extra[2].to_string(),
            w.extra[3].to_string(),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

/// Proposed default: the smallest swept `steps` whose mean `opt_moves` is within 2 % of the
/// largest mean over all swept values (the walk has saturated), among cells meeting the D3
/// criterion.
fn proposed_steps<'a>(rows: impl Iterator<Item = &'a Row> + Clone) -> Option<u32> {
    let best = rows
        .clone()
        .filter(|r| r.cell.supported())
        .map(|r| r.cell.opt_mean)
        .fold(f64::NAN, f64::max);
    rows.filter(|r| r.cell.supported() && r.cell.opt_mean >= 0.98 * best)
        .map(|r| r.steps)
        .min()
}

/// Uniform's mean `opt_moves` for `(layout, params)` from the committed stats reports.
fn uniform_opt_mean(layout: Layout, params: Params) -> Option<f64> {
    let csv = match layout {
        Layout::Standard => include_str!("../../reports/uniform_stats.csv"),
        Layout::Distributed => include_str!("../../reports/uniform_distributed_stats.csv"),
    };
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (c, k, e, m) = (
        col("n_colors")?,
        col("capacity")?,
        col("n_empty")?,
        col("opt_mean")?,
    );
    lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .find(|f| {
            f[c].parse() == Ok(params.n_colors)
                && f[k].parse() == Ok(params.capacity)
                && f[e].parse() == Ok(params.n_empty)
        })
        .and_then(|f| f[m].parse().ok())
}

#[allow(clippy::cast_precision_loss)]
fn pct(count: u32, total: u32) -> String {
    if total == 0 {
        String::new()
    } else {
        format!("{:.1}", 100.0 * f64::from(count) / f64::from(total))
    }
}

fn intro(args: &SweepArgs, threads: usize) -> String {
    let scramble = args.strategy == StrategyArg::Scramble;
    let mut md = String::from(match args.strategy {
        StrategyArg::PourWalk => "# Turan pour walk `steps` sweep\n\n",
        StrategyArg::ReverseSearch => "# Turan reverse search state-budget sweep\n\n",
        _ => "# Turan scramble `steps` sweep\n\n",
    });
    let steps: Vec<String> = args.steps.iter().map(u32::to_string).collect();
    let strategy = match args.strategy {
        StrategyArg::PourWalk => " --strategy pour-walk".to_string(),
        StrategyArg::ReverseSearch => format!(
            " --strategy reverse-search --search-depth {}",
            args.search_depth
        ),
        _ => String::new(),
    };
    let layouts = if scramble {
        String::new()
    } else {
        let names: Vec<String> = args
            .layouts
            .iter()
            .map(|&l| Layout::from(l).to_string())
            .collect();
        format!(" --layouts {}", names.join(","))
    };
    let _ = writeln!(
        md,
        "Produced by `water_sort_cli sweep{strategy}{layouts} --steps {} --samples {} --max-extra-steps {} \
         --rollouts {} --base-seed {}` (release build, {threads} threads; default `GenConfig`: \
         `min_opt` 1, `max_attempts` 10000, `max_states` 5e6). Configurations: every supported \
         one (D3) unless `--configs` is given.\n",
        steps.join(","),
        args.samples,
        args.max_extra_steps,
        args.rollouts,
        args.base_seed,
    );
    if args.strategy == StrategyArg::ReverseSearch {
        md.push_str(
            "`steps` is the search's distinct-state budget (`max_states`). `constr. %` is the \
             share of attempts where no visited state scored above 0 (D16). Walk columns do not \
             apply.\n\n",
        );
    } else if args.strategy == StrategyArg::PourWalk {
        md.push_str(
            "Every endpoint goes to the solver: unsolvable and already-solved endpoints are \
             rejected (`unsolv. %`, `solved/min %`). Walk columns do not apply.\n\n",
        );
    }
    md.push_str(
        "Generation columns are over the generated puzzles: `constr. %` is the share of attempts \
         rejected by the construction before solving (standard layout: the walk did not return; \
         D15), `solved/min %` the share rejected as already solved or below `min_opt`. Walk \
         columns are over the same number of independent scramble candidates: `dead end %` is \
         the share of walks absorbed in a state with no reverse move (a state no pour can lead \
         to), `no return %` the share that did not reach the standard layout, and `extra` the \
         extra steps of those that did (p50 / p99 / max).\n\n",
    );
    md
}

fn markdown(args: &SweepArgs, threads: usize, rows: &[Row]) -> String {
    let scramble = args.strategy == StrategyArg::Scramble;
    let mut md = intro(args, threads);
    if scramble {
        md.push_str("## Proposed default `steps` (D15, proposed)\n\n");
        md.push_str(
            "Smallest swept value whose mean `opt_moves` is within 2 % of the largest mean over \
             the sweep, among values meeting the D3 criterion.\n\n",
        );
        md.push_str("| layout | config | proposed steps | opt mean there | max opt mean |\n");
        md.push_str("|---|---|---:|---:|---:|\n");
    } else {
        md.push_str("## Saturation and uniform's mean (D16)\n\n");
        md.push_str(
            "`saturates at`: smallest swept value whose mean `opt_moves` is within 2 % of the \
             largest mean over the sweep, among values meeting the D3 criterion. `uniform` is \
             uniform's mean `opt_moves` for the same layout from the committed stats report \
             (`reports/uniform_stats.csv` / `reports/uniform_distributed_stats.csv`, other seeds \
             and 1000 samples).\n\n",
        );
        md.push_str(
            "| layout | config | saturates at | opt mean there | max opt mean | uniform | max / uniform |\n",
        );
        md.push_str("|---|---|---:|---:|---:|---:|---:|\n");
    }
    let mut groups: Vec<(Layout, Params)> =
        rows.iter().map(|r| (r.layout, r.cell.params)).collect();
    groups.dedup();
    for (layout, params) in &groups {
        let group = rows
            .iter()
            .filter(|r| r.layout == *layout && r.cell.params == *params);
        let best = group
            .clone()
            .map(|r| r.cell.opt_mean)
            .fold(f64::NAN, f64::max);
        let proposed = proposed_steps(group.clone());
        let at = proposed
            .and_then(|s| group.clone().find(|r| r.steps == s))
            .map_or(String::new(), |r| format!("{:.2}", r.cell.opt_mean));
        let _ = write!(
            md,
            "| {layout} | {}x{}x{} | {} | {at} | {best:.2} |",
            params.n_colors,
            params.capacity,
            params.n_empty,
            proposed.map_or_else(|| "none".into(), |s| s.to_string()),
        );
        if !scramble {
            let _ = match uniform_opt_mean(*layout, *params) {
                Some(u) => write!(md, " {u:.2} | {:.0} % |", 100.0 * best / u),
                None => write!(md, " | |"),
            };
        }
        md.push('\n');
    }
    md.push_str("\n## All cells\n\n");
    md.push_str(
        "| layout | config | steps | ok | attempts mean / p99 | constr. % | solved/min % \
         | opt mean / p50 / p99 | gen ms p50 / p99 | dead end % | no return % | extra p50 / p99 / max |\n",
    );
    md.push_str("|---|---|---:|:-:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for r in rows {
        let c = &r.cell;
        let j = &c.rejections;
        let w = &r.walk;
        let _ = writeln!(
            md,
            "| {} | {}x{}x{} | {} | {} | {:.2} / {} | {:.1} | {:.1} | {:.2} / {} / {} | {:.1} / {:.1} | {} | {} | {} / {} / {} |",
            r.layout,
            c.params.n_colors,
            c.params.capacity,
            c.params.n_empty,
            r.steps,
            if c.supported() { "yes" } else { "no" },
            c.attempts_mean,
            c.attempts_p99,
            c.rate(j.construction) * 100.0,
            c.rate(j.already_solved + j.below_min_opt + j.above_max_opt) * 100.0,
            c.opt_mean,
            c.opt_p50,
            c.opt_p99,
            c.gen_ms_p50,
            c.gen_ms_p99,
            pct(w.dead_ends, w.candidates),
            pct(w.not_returned, w.candidates),
            w.extra[0],
            w.extra[2],
            w.extra[3],
        );
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sweep() {
        let dir = std::env::temp_dir().join(format!("wsc_sweep_{}", std::process::id()));
        let args = SweepArgs {
            strategy: StrategyArg::Scramble,
            search_depth: 300,
            steps: vec![5, 40],
            layouts: vec![LayoutArg::Standard, LayoutArg::Distributed],
            configs: vec!["4x3x2".parse().unwrap()],
            samples: 30,
            max_extra_steps: 100,
            rollouts: 0,
            base_seed: 3,
            out: Some(dir.join("sweep")),
            threads: Some(2),
        };
        run(&args).unwrap();
        let csv = std::fs::read_to_string(dir.join("sweep.csv")).unwrap();
        assert_eq!(csv.lines().count(), 5);
        let md = std::fs::read_to_string(dir.join("sweep.md")).unwrap();
        assert!(md.contains("| standard | 4x3x2 |"));
        assert!(md.contains("| distributed | 4x3x2 | 40 |"));
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(
            SweepArgs {
                configs: vec![],
                ..args
            }
            .configs(Layout::Distributed)
            .len(),
            54
        );
    }

    #[test]
    fn walk_and_search_sweeps() {
        let dir = std::env::temp_dir().join(format!("wsc_sweep2_{}", std::process::id()));
        let base = SweepArgs {
            strategy: StrategyArg::PourWalk,
            search_depth: 300,
            steps: vec![10, 80],
            layouts: vec![LayoutArg::Distributed],
            configs: vec!["4x4x2".parse().unwrap()],
            samples: 20,
            max_extra_steps: 100,
            rollouts: 0,
            base_seed: 3,
            out: Some(dir.join("walk")),
            threads: Some(2),
        };
        run(&base).unwrap();
        let md = std::fs::read_to_string(dir.join("walk.md")).unwrap();
        assert!(md.contains("--strategy pour-walk --layouts distributed"));
        assert!(md.contains("| distributed | 4x4x2 |"));
        assert!(
            md.contains(" 10.6"),
            "uniform's distributed 4x4x2 mean is listed"
        );
        let standard = SweepArgs {
            layouts: vec![LayoutArg::Standard],
            ..base.clone()
        };
        assert!(run(&standard).is_err());
        let search = SweepArgs {
            strategy: StrategyArg::ReverseSearch,
            steps: vec![200],
            layouts: vec![LayoutArg::Standard],
            out: Some(dir.join("search")),
            ..base
        };
        run(&search).unwrap();
        let md = std::fs::read_to_string(dir.join("search.md")).unwrap();
        assert!(md.contains("--strategy reverse-search --search-depth 300"));
        assert!(md.contains("| standard | 4x4x2 | 200 |"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
