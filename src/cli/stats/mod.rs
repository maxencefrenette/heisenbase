mod material;
mod summary;
mod top;

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub(crate) enum StatsCommands {
    /// Show a one-screen summary of the current database.
    Summary(summary::SummaryArgs),
    /// Show the top indexed material keys from the PGN index.
    Top(top::TopArgs),
    /// Show detailed stats for a single material key.
    Material(material::MaterialArgs),
}

#[derive(Args)]
#[command(
    about = "Inspect statistics derived from the SQLite database",
    long_about = "Inspect statistics derived from the SQLite database. Subcommands are designed for plain, LLM-friendly output."
)]
pub(crate) struct StatsArgs {
    #[command(subcommand)]
    command: StatsCommands,
}

pub(crate) fn run(args: StatsArgs) -> Result<()> {
    match args.command {
        StatsCommands::Summary(args) => summary::run(args),
        StatsCommands::Top(args) => top::run(args),
        StatsCommands::Material(args) => material::run(args),
    }
}
