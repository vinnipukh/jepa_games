//! `water_sort_cli`: batch generation, validation and statistics commands.

mod compare;
mod stats;
mod sweep;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use water_sort_cli::args;
use water_sort_cli::dataset::commands;

#[derive(Parser)]
#[command(version, about = "Water Sort puzzle generation tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generation statistics per `(n_colors, capacity, n_empty)` cell, as CSV + Markdown.
    Stats(stats::StatsArgs),
    /// Two generators side by side: measurements, histograms, two-sample tests, overlap.
    Compare(compare::CompareArgs),
    /// Turan scramble `steps` sweep per layout, with construction costs.
    Sweep(sweep::SweepArgs),
    /// A puzzle dataset (Parquet or JSONL) with manifest, dedup report and split.
    Generate(commands::GenerateArgs),
    /// Duplicate, split and tier report of a dataset, with a fresh scan of its stored records.
    DedupReport(commands::DedupReportArgs),
    /// Puzzles shared between two datasets (by canonical form); optionally drops them from one.
    Leakage(commands::LeakageArgs),
    /// Re-checks every record of a dataset and regenerates a sample from its seed.
    Validate(commands::ValidateArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Stats(args) => stats::run(args).map(|_| ()),
        Command::Sweep(args) => sweep::run(args),
        Command::Generate(args) => commands::run_generate(args).map(|_| ()).map_err(Into::into),
        Command::DedupReport(args) => commands::run_dedup_report(args).map_err(Into::into),
        Command::Leakage(args) => commands::run_leakage(args).map_err(Into::into),
        Command::Validate(args) => match commands::run_validate(args) {
            Ok(true) => Ok(()),
            Ok(false) => Err("validation failed".into()),
            Err(e) => Err(e.into()),
        },
        Command::Compare(args) => compare::run(args).map(|path| {
            eprintln!("wrote {}", path.display());
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn dataset_commands_parse() {
        use water_sort_cli::dataset::Split;
        use water_sort_cli::dataset::commands::SplitSelection;
        let cli = Cli::parse_from([
            "water_sort_cli",
            "generate",
            "--count",
            "1e6",
            "--colors",
            "6",
            "--master-seed",
            "0x88f5e8812f916163",
            "--created-at",
            "2026-10-03T00:00:00Z",
            "--split-files",
            "--out",
            "data/x",
        ]);
        let Command::Generate(a) = cli.command else {
            panic!("expected generate")
        };
        assert_eq!(
            (a.count, a.master_seed),
            (1_000_000, Some(0x88f5_e881_2f91_6163))
        );
        let opts = a.options().unwrap();
        assert_eq!(opts.created_at, 1_790_985_600_000_000_000);
        assert!(opts.split_files && opts.supported);
        let cli = Cli::parse_from([
            "water_sort_cli",
            "generate",
            "--count",
            "5",
            "--colors",
            "12",
            "--out",
            "x",
        ]);
        let Command::Generate(a) = cli.command else {
            panic!("expected generate")
        };
        assert!(a.options().is_err(), "12 x 4 x 2 is unsupported");
        let cli = Cli::parse_from([
            "water_sort_cli",
            "leakage",
            "--a",
            "x",
            "--b",
            "y",
            "--b-split",
            "all",
            "--exclude-from",
            "b",
            "--out",
            "z",
        ]);
        let Command::Leakage(a) = cli.command else {
            panic!("expected leakage")
        };
        assert_eq!(a.a_split, SplitSelection(Some(Split::Test)));
        assert_eq!(a.b_split, SplitSelection(None));
        assert!(
            Cli::try_parse_from([
                "water_sort_cli",
                "leakage",
                "--a",
                "x",
                "--b",
                "y",
                "--exclude-from",
                "b"
            ])
            .is_err()
        );
        let cli = Cli::parse_from(["water_sort_cli", "validate", "d", "--regen-rate", "1"]);
        let Command::Validate(a) = cli.command else {
            panic!("expected validate")
        };
        assert!((a.regen_rate - 1.0).abs() < f64::EPSILON);
        let cli = Cli::parse_from(["water_sort_cli", "dedup-report", "d"]);
        assert!(matches!(cli.command, Command::DedupReport(_)));
    }

    #[test]
    fn cli_is_well_formed() {
        Cli::command().debug_assert();
        let cli = Cli::parse_from([
            "water_sort_cli",
            "stats",
            "--generator",
            "uniform",
            "--colors",
            "2..=12",
            "--max-states",
            "5e6",
            "--base-seed",
            "7",
        ]);
        let Command::Stats(a) = cli.command else {
            panic!("expected stats")
        };
        assert_eq!(a.colors, args::Range(2..=12));
        assert_eq!((a.max_states, a.base_seed), (5_000_000, 7));
        assert_eq!(a.generator.generator, args::GeneratorKind::Uniform);
        assert_eq!(a.out_path(), std::path::Path::new("reports/uniform_stats"));
        let cli = Cli::parse_from([
            "water_sort_cli",
            "stats",
            "--generator",
            "turan",
            "--strategy",
            "scramble",
            "--steps",
            "80",
            "--layout",
            "distributed",
        ]);
        let Command::Stats(a) = cli.command else {
            panic!("expected stats")
        };
        assert_eq!(a.spec().variant(), "scramble(steps=80,layout=distributed)");
        assert_eq!(
            a.out_path(),
            std::path::Path::new("reports").join("turan_scramble_distributed_stats")
        );
        let cli = Cli::parse_from([
            "water_sort_cli",
            "stats",
            "--generator",
            "turan",
            "--strategy",
            "reverse-search",
            "--search-states",
            "1e3",
        ]);
        let Command::Stats(a) = cli.command else {
            panic!("expected stats")
        };
        assert_eq!(
            a.spec().variant(),
            "reverse_search(max_depth=300,max_states=1000,layout=standard)"
        );
        let cli = Cli::parse_from([
            "water_sort_cli",
            "stats",
            "--generator",
            "turan",
            "--strategy",
            "pour-walk",
            "--layout",
            "distributed",
        ]);
        let Command::Stats(a) = cli.command else {
            panic!("expected stats")
        };
        assert_eq!(
            a.spec().variant(),
            "pour_walk(steps=160,layout=distributed)"
        );
    }
}
