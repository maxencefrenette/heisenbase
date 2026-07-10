mod generate;
mod index;
mod index_pgn;
mod stats;
mod syzygy;

use clap::Args;
use clap::{Parser, Subcommand};

use anyhow::{Result, anyhow};
use heisenbase::material_key::MaterialKey;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Generate and analyze a sparse SQLite-backed chess tablebase",
    long_about = "Generate and analyze a sparse SQLite-backed chess tablebase. Commands cover table generation, PGN indexing, Syzygy comparison, and database stats."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a table for a given material key.
    Generate {
        /// Material key describing pieces, e.g. `KQvK`.
        material_key: String,
    },
    /// Generate tables for many material keys that match the given filters.
    GenerateMany {
        /// Minimum number of games required in the index.
        #[arg(long, default_value_t = 100)]
        min_games: u64,
        /// Maximum total number of pieces allowed.
        #[arg(long, required = true)]
        max_pieces: u32,
    },
    /// Build the PGN index from fishtest PGN files.
    #[command(
        name = "pgn-index",
        about = "Build the PGN index used for ranking material keys",
        long_about = "Build the PGN index used for ranking material keys. By default this runs both phases: first parse fishtest PGNs into pgn_index_raw, then derive the filtered pgn_index table."
    )]
    PgnIndex(PgnIndexArgs),
    /// Sample positions from heisenbase tables and compare against Syzygy WDL tables.
    CheckAgainstSyzygy,
    /// Initialize the sqlite database.
    #[command(name = "index-init")]
    IndexInit,
    /// Show stats about the current SQLite database.
    Stats(stats::StatsArgs),
}

#[derive(Args)]
struct PgnIndexArgs {
    /// Skip PGN parsing and rebuild only the derived pgn_index table from pgn_index_raw.
    #[arg(long, conflicts_with = "stage1_only")]
    from_raw: bool,
    /// Run only the raw PGN parsing phase and stop after writing pgn_index_raw.
    #[arg(long, conflicts_with = "from_raw")]
    stage1_only: bool,
}

/// Parse CLI arguments and execute the requested command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    warn_if_debug_build(&cli.command);

    match cli.command {
        Commands::Generate { material_key } => {
            let material = MaterialKey::from_string(&material_key)
                .map_err(|err| anyhow!("invalid material key: {material_key}: {err}"))?;
            generate::run_generate(material)?;
        }
        Commands::GenerateMany {
            min_games,
            max_pieces,
        } => {
            generate::run_generate_many(min_games, max_pieces)?;
        }
        Commands::PgnIndex(args) => {
            if args.stage1_only {
                index_pgn::run_stage1()?;
            } else if args.from_raw {
                index_pgn::run_stage2()?;
            } else {
                index_pgn::run_stage1()?;
                index_pgn::run_stage2()?;
            }
        }
        Commands::CheckAgainstSyzygy => {
            syzygy::run()?;
        }
        Commands::IndexInit => {
            index::run_index_init()?;
        }
        Commands::Stats(args) => {
            stats::run(args)?;
        }
    }

    Ok(())
}

fn warn_if_debug_build(command: &Commands) {
    if cfg!(debug_assertions) && command.is_long_running() {
        eprintln!(
            "Warning: this command is running in debug mode and may be very slow. Use a release build (`cargo run --release -- ...`) for long-running CLI commands."
        );
    }
}

impl Commands {
    fn is_long_running(&self) -> bool {
        matches!(
            self,
            Self::Generate { .. }
                | Self::GenerateMany { .. }
                | Self::PgnIndex(_)
                | Self::CheckAgainstSyzygy
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, PgnIndexArgs};
    use clap::{Arg, Command, CommandFactory};

    #[test]
    fn classifies_long_running_commands() {
        assert!(
            Commands::Generate {
                material_key: "KQvK".to_string(),
            }
            .is_long_running()
        );
        assert!(
            Commands::GenerateMany {
                min_games: 100,
                max_pieces: 4,
            }
            .is_long_running()
        );
        assert!(
            Commands::PgnIndex(PgnIndexArgs {
                from_raw: false,
                stage1_only: false,
            })
            .is_long_running()
        );
        assert!(Commands::CheckAgainstSyzygy.is_long_running());
        assert!(!Commands::IndexInit.is_long_running());
    }

    #[test]
    fn every_command_has_help_text() {
        let command = Cli::command();
        assert_command_help(&command, "heisenbase");
    }

    #[test]
    fn every_argument_has_help_text() {
        let command = Cli::command();
        assert_argument_help(&command, "heisenbase");
    }

    fn assert_command_help(command: &Command, path: &str) {
        let has_help = command.get_about().is_some() || command.get_long_about().is_some();
        assert!(has_help, "command `{path}` is missing help text");

        for subcommand in command.get_subcommands() {
            if subcommand.get_name() == "help" {
                continue;
            }
            let sub_path = format!("{path} {}", subcommand.get_name());
            assert_command_help(subcommand, &sub_path);
        }
    }

    fn assert_argument_help(command: &Command, path: &str) {
        for arg in command.get_arguments() {
            if is_auto_arg(arg) {
                continue;
            }
            let has_help = arg.get_help().is_some() || arg.get_long_help().is_some();
            assert!(
                has_help,
                "argument `{}` on command `{path}` is missing help text",
                arg.get_id()
            );
        }

        for subcommand in command.get_subcommands() {
            if subcommand.get_name() == "help" {
                continue;
            }
            let sub_path = format!("{path} {}", subcommand.get_name());
            assert_argument_help(subcommand, &sub_path);
        }
    }

    fn is_auto_arg(arg: &Arg) -> bool {
        matches!(arg.get_id().as_str(), "help" | "version")
    }
}
