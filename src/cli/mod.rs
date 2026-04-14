mod generate;
mod index;
mod index_pgn;
mod stats;

use clap::{Parser, Subcommand};
use rand::{Rng, SeedableRng, rngs::StdRng};
use shakmaty::{Chess, EnPassantMode, fen::Fen};
use shakmaty_syzygy::{SyzygyError, Tablebase, Wdl};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use heisenbase::material_key::MaterialKey;
use heisenbase::position_indexer::PositionIndexer;
use heisenbase::storage::Database;
use heisenbase::wdl_score_range::WdlScoreRange;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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
    /// Index fishtest PGN files into the sqlite raw PGN index.
    PgnIndexStage1,
    /// Build the filtered sqlite PGN index with derived columns.
    PgnIndexStage2,
    /// Sample positions from heisenbase tables and compare against Syzygy WDL tables.
    CheckAgainstSyzygy,
    /// Initialize the sqlite database.
    #[command(name = "index-init")]
    IndexInit,
    /// Show stats about the current SQLite database.
    Stats(stats::StatsArgs),
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
        Commands::PgnIndexStage1 => {
            index_pgn::run_stage1()?;
        }
        Commands::PgnIndexStage2 => {
            index_pgn::run_stage2()?;
        }
        Commands::CheckAgainstSyzygy => {
            run_check_against_syzygy()?;
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
                | Self::PgnIndexStage1
                | Self::PgnIndexStage2
                | Self::CheckAgainstSyzygy
        )
    }
}

const SAMPLES_PER_TABLE: usize = 256;
const MAX_MISMATCHES_PER_TABLE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimpleWdl {
    Win,
    Draw,
    Loss,
}

fn simplify_wdl(wdl: Wdl) -> SimpleWdl {
    match wdl {
        Wdl::Win | Wdl::CursedWin => SimpleWdl::Win,
        Wdl::Draw => SimpleWdl::Draw,
        Wdl::Loss | Wdl::BlessedLoss => SimpleWdl::Loss,
    }
}

fn heisenbase_allows(wdl: WdlScoreRange, syzygy: SimpleWdl) -> bool {
    match wdl {
        WdlScoreRange::Win => syzygy == SimpleWdl::Win,
        WdlScoreRange::Draw => syzygy == SimpleWdl::Draw,
        WdlScoreRange::Loss => syzygy == SimpleWdl::Loss,
        WdlScoreRange::WinOrDraw => matches!(syzygy, SimpleWdl::Win | SimpleWdl::Draw),
        WdlScoreRange::DrawOrLoss => matches!(syzygy, SimpleWdl::Draw | SimpleWdl::Loss),
        WdlScoreRange::Unknown => true,
        WdlScoreRange::IllegalPosition => false,
    }
}

fn material_keys_from_db() -> Result<Vec<MaterialKey>> {
    let db = Database::open_default()?;
    db.list_wdl_table_keys()
}

fn collect_valid_indices(indexer: &PositionIndexer) -> Vec<usize> {
    let total = indexer.total_positions();
    let mut valid = Vec::new();
    for idx in 0..total {
        if indexer.index_to_position(idx).is_ok() {
            valid.push(idx);
        }
    }
    valid
}

fn run_check_against_syzygy() -> Result<()> {
    let syzygy_dir = Path::new("./data/syzygy");
    let db = Database::open_default()?;

    let mut tablebase = Tablebase::<Chess>::new();
    let added = tablebase.add_directory(syzygy_dir)?;
    if added == 0 {
        bail!("no syzygy tables found in {}", syzygy_dir.display());
    }

    let all_keys = material_keys_from_db()?;
    let mut three_man = Vec::new();
    let mut four_man = Vec::new();
    for key in all_keys {
        match key.total_piece_count() {
            3 => three_man.push(key),
            4 => four_man.push(key),
            _ => (),
        }
    }

    let mut rng = StdRng::from_entropy();
    let mut total_tables = 0usize;
    let mut total_positions = 0usize;
    let mut total_mismatches = 0usize;
    let mut total_uncertain = 0usize;
    let mut missing_tables = 0usize;
    let mut probe_errors = 0usize;

    for (label, keys) in [("3-man", three_man), ("4-man", four_man)] {
        println!(
            "Checking {} material keys ({} tables)...",
            label,
            keys.len()
        );
        for material in keys {
            total_tables += 1;
            let Some(table) = db.get_wdl_table(&material)? else {
                eprintln!("Missing heisenbase table for {}", material);
                continue;
            };
            let indexer = PositionIndexer::new(material.clone());
            let valid_indices = collect_valid_indices(&indexer);
            if valid_indices.is_empty() {
                eprintln!("No valid positions for {}", material);
                continue;
            }

            let mut mismatches = 0usize;
            let mut uncertain = 0usize;
            let mut missing_table = false;
            let mut probe_failed = false;

            for _ in 0..SAMPLES_PER_TABLE {
                let idx = valid_indices[rng.gen_range(0..valid_indices.len())];
                let pos = match indexer.index_to_position(idx) {
                    Ok(pos) => pos,
                    Err(_) => continue,
                };

                let hb_wdl = table.positions[idx];
                if hb_wdl.is_uncertain() {
                    uncertain += 1;
                }

                let syzygy_wdl = match tablebase.probe_wdl_after_zeroing(&pos) {
                    Ok(wdl) => wdl,
                    Err(SyzygyError::MissingTable { .. }) => {
                        missing_table = true;
                        break;
                    }
                    Err(_) => {
                        probe_failed = true;
                        break;
                    }
                };

                let syzygy_simple = simplify_wdl(syzygy_wdl);
                if !heisenbase_allows(hb_wdl, syzygy_simple) {
                    mismatches += 1;
                    if mismatches <= MAX_MISMATCHES_PER_TABLE {
                        let fen = Fen::from_position(&pos, EnPassantMode::Legal).to_string();
                        println!(
                            "Mismatch {}: hb={:?}, syzygy={:?}, fen={}",
                            material, hb_wdl, syzygy_wdl, fen
                        );
                    }
                }
            }

            if missing_table {
                missing_tables += 1;
                eprintln!("Missing Syzygy tables for {}", material);
                continue;
            }
            if probe_failed {
                probe_errors += 1;
                eprintln!("Syzygy probe failed for {}", material);
                continue;
            }

            total_positions += SAMPLES_PER_TABLE;
            total_mismatches += mismatches;
            total_uncertain += uncertain;

            if mismatches > 0 {
                println!(
                    "Found {} mismatches in {} ({} uncertain samples).",
                    mismatches, material, uncertain
                );
            }
        }
    }

    println!(
        "Checked {} tables ({} positions).",
        total_tables, total_positions
    );
    println!("Mismatches: {}", total_mismatches);
    println!("Uncertain samples: {}", total_uncertain);
    if missing_tables > 0 {
        println!("Missing Syzygy tables: {}", missing_tables);
    }
    if probe_errors > 0 {
        println!("Syzygy probe errors: {}", probe_errors);
    }

    if total_mismatches > 0 || missing_tables > 0 || probe_errors > 0 {
        bail!("syzygy comparison reported mismatches or errors");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Commands;

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
        assert!(Commands::PgnIndexStage1.is_long_running());
        assert!(Commands::PgnIndexStage2.is_long_running());
        assert!(Commands::CheckAgainstSyzygy.is_long_running());
        assert!(!Commands::IndexInit.is_long_running());
    }
}
