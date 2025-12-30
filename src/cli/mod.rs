mod generate;
mod index;
mod index_pgn;

use clap::{Parser, Subcommand};
use rand::{Rng, SeedableRng, rngs::StdRng};
use shakmaty::{Chess, EnPassantMode, fen::Fen};
use shakmaty_syzygy::{SyzygyError, Tablebase, Wdl};
use std::{collections::HashSet, fs, path::Path};

use anyhow::{Result, anyhow, bail};
use heisenbase::material_key::MaterialKey;
use heisenbase::position_indexer::PositionIndexer;
use heisenbase::wdl_file::read_wdl_file;
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
    /// Index fishtest PGN files into pgn_index_raw.parquet.
    PgnIndexStage1,
    /// Build the filtered PGN index with derived columns.
    PgnIndexStage2,
    /// Sample positions from heisenbase tables and compare against Syzygy WDL tables.
    CheckAgainstSyzygy,
    /// Initialize the DuckDB material key index.
    #[command(name = "ìndex-init")]
    IndexInit,
}

/// Parse CLI arguments and execute the requested command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

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
    }

    Ok(())
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

fn material_keys_from_dir(dir: &Path) -> Result<Vec<MaterialKey>> {
    let mut keys = HashSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("hbt") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(key) = MaterialKey::from_string(stem) else {
            eprintln!(
                "Skipping unrecognized material key file: {}",
                path.display()
            );
            continue;
        };
        keys.insert(key);
    }

    let mut keys: Vec<MaterialKey> = keys.into_iter().collect();
    keys.sort();
    Ok(keys)
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
    let heisenbase_dir = Path::new("./data/heisenbase");
    let syzygy_dir = Path::new("./data/syzygy");

    let mut tablebase = Tablebase::<Chess>::new();
    let added = tablebase.add_directory(syzygy_dir)?;
    if added == 0 {
        bail!("no syzygy tables found in {}", syzygy_dir.display());
    }

    let all_keys = material_keys_from_dir(heisenbase_dir)?;
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
            let table_path = heisenbase_dir.join(format!("{}.hbt", material));
            let table = read_wdl_file(&table_path)?;
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
