//! `water_sort_cli`: batch generation, validation and statistics commands.

mod args;
mod compare;
mod stats;
mod sweep;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Stats(args) => stats::run(args).map(|_| ()),
        Command::Sweep(args) => sweep::run(args),
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
