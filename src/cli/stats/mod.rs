mod coverage;
mod frontier;
mod material;
mod piece_histogram;
mod size_curve;
mod summary;
mod top;
mod utility;

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub(crate) enum StatsCommands {
    /// Show exact-label coverage overall and by piece count.
    Coverage(coverage::CoverageArgs),
    /// Show the material keys that generate-many would process next.
    Frontier(frontier::FrontierArgs),
    /// Show a one-screen summary of the current database.
    Summary(summary::SummaryArgs),
    /// Show the top indexed material keys from the PGN index.
    Top(top::TopArgs),
    /// Explain direct and transitive utility for one material key.
    Utility(utility::UtilityArgs),
    /// Show detailed stats for a single material key.
    Material(material::MaterialArgs),
    /// Show the piece-count distribution under a tablebase size budget.
    PieceHistogram(piece_histogram::PieceHistogramArgs),
    /// Show PGN coverage as a utility-ranked tablebase grows.
    SizeCurve(size_curve::SizeCurveArgs),
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
        StatsCommands::Coverage(args) => coverage::run(args),
        StatsCommands::Frontier(args) => frontier::run(args),
        StatsCommands::Summary(args) => summary::run(args),
        StatsCommands::Top(args) => top::run(args),
        StatsCommands::Utility(args) => utility::run(args),
        StatsCommands::Material(args) => material::run(args),
        StatsCommands::PieceHistogram(args) => piece_histogram::run(args),
        StatsCommands::SizeCurve(args) => size_curve::run(args),
    }
}
