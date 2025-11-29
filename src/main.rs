mod index_pgn;

use clap::{Parser, Subcommand};
use polars::prelude::{ParquetReader, SerReader};
use std::{error::Error, fs::File, io, path::Path};

use heisenbase::compression::compress_wdl;
use heisenbase::material_key::MaterialKey;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_file::write_wdl_file;
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
    let compressed = compress_wdl(&wdl_table.positions);
    let filename = format!("./data/heisenbase/{}.hbt", wdl_table.material);
    write_wdl_file(&filename, &wdl_table.material, &compressed)?;
    println!("Wrote table to {}", filename);
    Ok(())
}

fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<(), Box<dyn Error>> {
    let df = ParquetReader::new(File::open(index_pgn::PARQUET_PATH)?)
        .finish()
        .expect("invalid parquet data");

    let keys = df
        .column("material_key")
        .expect("missing material_key column")
        .str()
        .expect("material_key not utf8")
        .into_iter();
    let games = df
        .column("num_games")
        .expect("missing num_games column")
        .u64()
        .expect("num_games not u64")
        .into_iter();

    let mut candidates = Vec::new();
    for (key, count) in keys.zip(games) {
        let count = count.expect("num_games null");
        if count < min_games {
            continue;
        }
        let material = MaterialKey::from_string(key.expect("material_key null"))
            .expect("invalid material key");
        if material.total_piece_count() > max_pieces {
            continue;
        }
        candidates.push((material, count));
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

    for (material, count) in candidates {
        let material_str = material.to_string();
        let filename = format!("./data/heisenbase/{}.hbt", material_str);
        if Path::new(&filename).exists() {
            println!("Skipping {} (already exists)", material_str);
            continue;
        }
        println!("Generating {} ({} games)", material_str, count);
        if let Err(err) = run_generate(material) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to generate {}: {}", material_str, err),
            )
            .into());
        }
    }

    Ok(())
}
