mod index_pgn;

use clap::{Parser, Subcommand};
use polars::prelude::*;
use rand::{Rng, SeedableRng, rngs::StdRng};
use shakmaty::{Chess, EnPassantMode, fen::Fen};
use shakmaty_syzygy::{SyzygyError, Tablebase, Wdl};
use std::{collections::HashSet, error::Error, fs, io, path::Path};

use heisenbase::material_key::MaterialKey;
use heisenbase::position_indexer::PositionIndexer;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_file::{read_wdl_file, write_wdl_file};
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;

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
    /// Index PGN files to find the most common material keys.
    IndexPgn,
    /// Sample positions from heisenbase tables and compare against Syzygy WDL tables.
    CheckAgainstSyzygy,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { material_key } => {
            let material = MaterialKey::from_string(&material_key).expect("invalid material key");
            if let Err(err) = run_generate(material) {
                eprintln!("generate failed: {err}");
                std::process::exit(1);
            }
        }
        Commands::GenerateMany {
            min_games,
            max_pieces,
        } => {
            if let Err(err) = run_generate_many(min_games, max_pieces) {
                eprintln!("generate-many failed: {err}");
                std::process::exit(1);
            }
        }
        Commands::IndexPgn => {
            if let Err(err) = index_pgn::run() {
                eprintln!("index-pgn failed: {err}");
                std::process::exit(1);
            }
        }
        Commands::CheckAgainstSyzygy => {
            if let Err(err) = run_check_against_syzygy() {
                eprintln!("check-against-syzygy failed: {err}");
                std::process::exit(1);
            }
        }
    }
}

fn run_generate(material: MaterialKey) -> io::Result<()> {
    let mut table_builder = TableBuilder::new(material);
    let loaded: Vec<String> = table_builder
        .loaded_child_materials()
        .iter()
        .map(|k| k.to_string())
        .collect();
    let missing: Vec<String> = table_builder
        .missing_child_materials()
        .iter()
        .map(|k| k.to_string())
        .collect();
    println!(
        "Loaded child tables: {}",
        if loaded.is_empty() {
            "(none)".to_string()
        } else {
            loaded.join(", ")
        }
    );
    println!(
        "Missing child tables: {}",
        if missing.is_empty() {
            "(none)".to_string()
        } else {
            missing.join(", ")
        }
    );
    table_builder.solve();
    let wdl_table: WdlTable = table_builder.into();
    let total = wdl_table.positions.len() as f64;
    let mut counts = [0usize; 7];
    for wdl in &wdl_table.positions {
        counts[*wdl as usize] += 1;
    }
    println!("WDL statistics:");
    for variant in [
        WdlScoreRange::Unknown,
        WdlScoreRange::WinOrDraw,
        WdlScoreRange::DrawOrLoss,
        WdlScoreRange::Win,
        WdlScoreRange::Draw,
        WdlScoreRange::Loss,
        WdlScoreRange::IllegalPosition,
    ] {
        let count = counts[variant as usize];
        let percentage = if total > 0.0 {
            (count as f64 / total) * 100.0
        } else {
            0.0
        };
        println!("{variant:?}: {percentage:.2}%");
    }
    let filename = format!("./data/heisenbase/{}.hbt", wdl_table.material);
    write_wdl_file(&filename, &wdl_table)?;
    println!("Wrote table to {}", filename);
    println!();
    Ok(())
}

fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<(), Box<dyn Error>> {
    let df = LazyFrame::scan_parquet(index_pgn::PARQUET_PATH, Default::default())
        .unwrap()
        .filter(col("num_games").gt(1))
        .with_columns([
            (lit(1_000_000_000f64) * col("num_positions").cast(DataType::Float64)
                / col("total_positions").cast(DataType::Float64)
                / col("material_key_size").cast(DataType::Float64))
            .alias("utility"),
        ])
        .sort(
            ["utility"],
            SortMultipleOptions::new().with_order_descending(true),
        )
        .collect()
        .unwrap();

    let keys = df.column("material_key").unwrap();

    let mut candidates = Vec::new();
    for key in keys.str().unwrap().into_iter() {
        let material_key = MaterialKey::from_string(key.expect("material_key null"))
            .expect("invalid material key");
        if material_key.total_piece_count() > max_pieces {
            continue;
        }
        candidates.push(material_key);
    }

    if candidates.is_empty() {
        println!(
            "No material keys matched filters (min-games: {}, max-pieces: {}).",
            min_games, max_pieces
        );
        return Ok(());
    }

    println!(
        "Generating {} material keys (min-games: {}, max-pieces: {}).",
        candidates.len(),
        min_games,
        max_pieces
    );

    for material_key in candidates {
        let material_str = material_key.to_string();
        let filename = format!("./data/heisenbase/{}.hbt", material_str);
        if Path::new(&filename).exists() {
            println!("Skipping {} (already exists)", material_str);
            continue;
        }
        println!("Generating {}", material_str);
        if let Err(err) = run_generate(material_key) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to generate {}: {}", material_str, err),
            )
            .into());
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

fn material_keys_from_dir(dir: &Path) -> Result<Vec<MaterialKey>, Box<dyn Error>> {
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
        let Some(key) = MaterialKey::from_string(stem) else {
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

fn run_check_against_syzygy() -> Result<(), Box<dyn Error>> {
    let heisenbase_dir = Path::new("./data/heisenbase");
    let syzygy_dir = Path::new("./data/syzygy");

    let mut tablebase = Tablebase::<Chess>::new();
    let added = tablebase.add_directory(syzygy_dir)?;
    if added == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no syzygy tables found in {}", syzygy_dir.display()),
        )
        .into());
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
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "syzygy comparison reported mismatches or errors",
        )
        .into());
    }

    Ok(())
}
