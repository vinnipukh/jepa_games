//! `water_sort_cli`: batch generation, validation and statistics commands.

mod args;
mod stats;

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Stats(args) => stats::run(args).map(|_| ()),
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
        let Command::Stats(a) = cli.command;
        assert_eq!(a.colors, args::Range(2..=12));
        assert_eq!((a.max_states, a.base_seed), (5_000_000, 7));
        assert_eq!(a.generator, args::GeneratorKind::Uniform);
    }
}
