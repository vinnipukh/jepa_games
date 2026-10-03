//! Command-line front ends of the dataset subcommands.

use std::io;
use std::path::PathBuf;

use clap::Args;
use water_sort_core::{GenConfig, MetricsConfig, Params, Tier, is_supported_in};

use super::dedup::standalone_report;
use super::generate::{CHUNK, GenerateOptions, WINDOW_CHUNKS, generate, has_dataset};
use super::manifest::{Format, Manifest, tool_version};
use super::split::SplitRanges;
use super::time::{now_unix_nanos, parse_rfc3339};
use crate::args::{GenArgs, parse_count};

/// `generate`: one generator, one configuration, `count` puzzles.
#[derive(Args, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // independent command-line switches
pub struct GenerateArgs {
    #[command(flatten)]
    pub generator: GenArgs,
    /// Indices to generate (accepts `1e6`); duplicates are dropped afterwards.
    #[arg(long, value_parser = parse_count)]
    pub count: u64,
    #[arg(long)]
    pub colors: u8,
    #[arg(long, default_value_t = Params::DEFAULT_CAPACITY)]
    pub capacity: u8,
    #[arg(long, default_value_t = Params::DEFAULT_N_EMPTY)]
    pub empty: u8,
    #[arg(long, default_value_t = GenConfig::default().min_opt)]
    pub min_opt: u32,
    /// Reject puzzles with `opt_moves` above this (D16).
    #[arg(long)]
    pub max_opt: Option<u32>,
    /// Generate only one difficulty tier (D16): sets `--min-opt` / `--max-opt` from the core
    /// tier table. Every record stores its tier anyway, so this is only for tier-only datasets.
    #[arg(long, conflicts_with_all = ["min_opt", "max_opt"])]
    pub tier: Option<Tier>,
    /// Solver state-count limit (accepts `5e6`).
    #[arg(long, value_parser = parse_count, default_value = "5000000")]
    pub max_states: u64,
    #[arg(long, default_value_t = GenConfig::default().max_attempts)]
    pub max_attempts: u32,
    /// Random rollouts per puzzle (metrics only).
    #[arg(long, default_value_t = MetricsConfig::default().random_rollouts)]
    pub rollouts: u32,
    /// Also compute the depth-1/2 dead-end ratios (slow: dozens of solves per puzzle).
    #[arg(long)]
    pub dead_end_ratios: bool,
    /// Record `i` uses seed `splitmix64(master_seed ^ i)`. Hex with `0x` or decimal. Default: the
    /// generator's fresh seed (OS entropy for uniform, the time seed for Turan).
    #[arg(long, value_parser = parse_seed)]
    pub master_seed: Option<u64>,
    /// Output directory.
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value = "parquet")]
    pub format: Format,
    /// Split percentages `train,val,test` by `canonical_hash % 100`.
    #[arg(long, default_value = "80,10,10")]
    pub split: SplitRanges,
    /// Write one file per split (`train.parquet`, `val.parquet`, `test.parquet`) instead of
    /// one `puzzles.parquet`.
    #[arg(long)]
    pub split_files: bool,
    /// Pin every record's `created_at` (RFC 3339, e.g. `2026-10-03T00:00:00Z`) instead of the
    /// run's start time, so two runs are byte-identical.
    #[arg(long, value_parser = parse_rfc3339)]
    pub created_at: Option<i64>,
    /// Allow params outside the supported range of the layout (D3).
    #[arg(long)]
    pub allow_unsupported: bool,
    /// Worker threads (default: `RAYON_NUM_THREADS`, else all cores).
    #[arg(long)]
    pub threads: Option<usize>,
    /// Overwrite an existing dataset in `--out`.
    #[arg(long)]
    pub force: bool,
}

/// `0x`-prefixed hex or decimal.
pub fn parse_seed(s: &str) -> Result<u64, String> {
    let parsed = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16),
        None => s.replace('_', "").parse(),
    };
    parsed.map_err(|e| format!("invalid seed {s:?}: {e}"))
}

impl GenerateArgs {
    pub fn params(&self) -> Params {
        Params {
            n_colors: self.colors,
            capacity: self.capacity,
            n_empty: self.empty,
        }
    }

    /// The generation config, with `--tier` resolved.
    pub fn gen_config(&self) -> Result<GenConfig, String> {
        let layout = self.generator.spec().layout();
        let (min_opt, max_opt) = match self.tier {
            Some(t) => t.opt_band(&self.params(), layout).ok_or_else(|| {
                format!(
                    "--tier needs a supported configuration; {:?} ({layout}) has no tier table",
                    self.params()
                )
            })?,
            None => (self.min_opt, self.max_opt),
        };
        Ok(GenConfig {
            min_opt,
            max_opt,
            max_attempts: self.max_attempts,
            max_states: self.max_states,
            metrics: MetricsConfig {
                random_rollouts: self.rollouts,
                dead_end_ratios: self.dead_end_ratios,
                ..MetricsConfig::default()
            },
        })
    }

    /// Checks the arguments and resolves the master seed and clock reading.
    pub fn options(&self) -> Result<GenerateOptions, String> {
        let spec = self.generator.spec();
        let params = self.params();
        params.validate().map_err(|e| e.to_string())?;
        let supported = is_supported_in(&params, spec.layout());
        if !supported && !self.allow_unsupported {
            return Err(format!(
                "{} colors / capacity {} / {} empty is outside the supported range of the {} \
                 layout (D3); pass --allow-unsupported to generate it anyway",
                params.n_colors,
                params.capacity,
                params.n_empty,
                spec.layout()
            ));
        }
        let now = now_unix_nanos();
        let (master_seed, source) = match self.master_seed {
            Some(s) => (s, "given"),
            None => (
                spec.fresh_seed(u64::try_from(now).unwrap_or(0))
                    .map_err(|e| e.to_string())?,
                "fresh",
            ),
        };
        Ok(GenerateOptions {
            spec,
            params,
            gen_config: self.gen_config()?,
            count: self.count,
            master_seed,
            master_seed_source: source.into(),
            split: self.split,
            split_files: self.split_files,
            format: self.format,
            // Whole microseconds, the Parquet timestamp resolution.
            created_at: {
                let t = self.created_at.unwrap_or(now);
                t - t.rem_euclid(1000)
            },
            tool_version: tool_version(),
            supported,
            threads: self.threads,
            progress: true,
            chunk: CHUNK,
            window_chunks: WINDOW_CHUNKS,
        })
    }
}

pub fn run_generate(args: &GenerateArgs) -> io::Result<Manifest> {
    let opts = args.options().map_err(io::Error::other)?;
    if has_dataset(&args.out) && !args.force {
        return Err(io::Error::other(format!(
            "{} already holds a dataset; pass --force to overwrite it",
            args.out.display()
        )));
    }
    if has_dataset(&args.out) {
        // Remove the old record files so none of them is left behind under a stale name.
        if let Ok(old) = Manifest::read(&args.out) {
            for f in &old.files {
                let _ = std::fs::remove_file(args.out.join(&f.path));
            }
        }
    }
    eprintln!(
        "generating {} puzzles: {} {} {:?}, master seed {:#018x} ({})",
        opts.count,
        opts.spec.id(),
        opts.spec.variant(),
        opts.params,
        opts.master_seed,
        opts.master_seed_source
    );
    let manifest = generate(&args.out, &opts)?;
    eprintln!(
        "wrote {} records to {} in {:.1} s ({} threads)",
        manifest.records,
        args.out.display(),
        manifest.wall_time_secs,
        manifest.threads
    );
    Ok(manifest)
}

/// `dedup-report <dir>`.
#[derive(Args, Debug, Clone)]
pub struct DedupReportArgs {
    /// Dataset directory (with `manifest.json`).
    pub dir: PathBuf,
    /// Write the Markdown report here instead of stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub fn run_dedup_report(args: &DedupReportArgs) -> io::Result<()> {
    let md = standalone_report(&args.dir)?;
    match &args.out {
        Some(path) => {
            std::fs::write(path, md)?;
            eprintln!("wrote {}", path.display());
        }
        None => print!("{md}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds() {
        assert_eq!(parse_seed("0x10"), Ok(16));
        assert_eq!(parse_seed("0xDEAD_beef"), Ok(0xdead_beef));
        assert_eq!(parse_seed("42"), Ok(42));
        assert!(parse_seed("0xg").is_err());
        assert!(parse_seed("-1").is_err());
    }
}
