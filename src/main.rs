mod index_pgn;

use clap::{Parser, Subcommand};
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
    /// Index PGN files to find the most common material keys.
    IndexPgn,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { material_key } => {
            let material = MaterialKey::from_string(&material_key).expect("invalid material key");
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
                println!("{:?}: {:.2}%", variant, percentage);
            }
            let compressed = compress_wdl(&wdl_table.positions);
            let filename = format!("./data/heisenbase/{}.hbt", wdl_table.material);
            write_wdl_file(&filename, &wdl_table.material, &compressed)
                .expect("failed to write table file");
            println!("Wrote table to {}", filename);
        }
        Commands::IndexPgn => {
            if let Err(err) = index_pgn::run() {
                eprintln!("index-pgn failed: {err}");
                std::process::exit(1);
            }
        }
    }
}
